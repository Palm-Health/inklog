// Copyright (c) 2026 Kirky.X
//
// Licensed under the MIT License
// See LICENSE file in the project root for full license information.

use crate::config::ConsoleSinkConfig;
use crate::error::InklogError;
use crate::log_record::LogRecord;
use crate::masking::DataMasker;
use crate::sink::LogSink;
use crate::template::LogTemplate;
use is_terminal::IsTerminal;
use owo_colors::OwoColorize;
use std::fmt;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

pub struct ConsoleSink {
    config: ConsoleSinkConfig,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    template: LogTemplate,
    masker: DataMasker,
}

impl fmt::Debug for ConsoleSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConsoleSink")
            .field("config", &self.config)
            .field("template", &self.template)
            .finish()
    }
}

impl ConsoleSink {
    pub fn new(config: ConsoleSinkConfig, template: LogTemplate) -> Self {
        Self {
            config,
            writer: Arc::new(Mutex::new(Box::new(io::stdout()))),
            template,
            masker: DataMasker::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn set_writer_for_test(&mut self, writer: Box<dyn Write + Send>) {
        self.writer = Arc::new(Mutex::new(writer));
    }

    fn write_record<W: Write>(
        &self,
        writer: &mut W,
        record: &LogRecord,
        use_color: bool,
    ) -> io::Result<()> {
        let formatted_message = self.template.render(record);

        if use_color {
            let level_colored = match record.level.as_str() {
                "ERROR" | "error" => record.level.red().to_string(),
                "WARN" | "warn" => record.level.yellow().to_string(),
                "INFO" | "info" => record.level.green().to_string(),
                "DEBUG" | "debug" => record.level.blue().to_string(),
                "TRACE" | "trace" => record.level.magenta().to_string(),
                _ => record.level.clone(),
            };
            writeln!(
                writer,
                "{}",
                self.apply_color(&formatted_message, &level_colored)
            )
        } else {
            writeln!(writer, "{}", formatted_message)
        }
    }

    fn apply_color(&self, message: &str, level: &str) -> String {
        match level {
            "ERROR" | "error" => message.red().to_string(),
            "WARN" | "warn" => message.yellow().to_string(),
            "INFO" | "info" => message.green().to_string(),
            "DEBUG" | "debug" => message.blue().to_string(),
            "TRACE" | "trace" => message.magenta().to_string(),
            _ => message.green().to_string(),
        }
    }

    fn should_colorize(&self, is_stderr: bool) -> bool {
        if !self.config.colored {
            return false;
        }

        // NO_COLOR standard (https://no-color.org/)
        if std::env::var("NO_COLOR").is_ok() {
            return false;
        }

        // FORCE_COLOR standard
        if let Ok(val) = std::env::var("CLICOLOR_FORCE") {
            if val != "0" {
                return true;
            }
        }

        // TERM=dumb
        if let Ok(term) = std::env::var("TERM") {
            if term == "dumb" {
                return false;
            }
        }

        if is_stderr {
            io::stderr().is_terminal()
        } else {
            io::stdout().is_terminal()
        }
    }
}

impl LogSink for ConsoleSink {
    fn write(&mut self, record: &LogRecord) -> Result<(), InklogError> {
        // 应用数据脱敏（如果启用）
        let masked_record = if self.config.masking_enabled {
            let mut masked = record.clone();
            masked.message = self.masker.mask(&record.message);
            self.masker.mask_hashmap(&mut masked.fields);
            masked
        } else {
            record.clone()
        };

        // Stderr separation
        let is_stderr = self
            .config
            .stderr_levels
            .contains(&masked_record.level.to_lowercase());

        let use_color = self.should_colorize(is_stderr);

        if is_stderr {
            let mut stderr = io::stderr();
            self.write_record(&mut stderr, &masked_record, use_color)
                .map_err(InklogError::IoError)?;
        } else {
            let mut writer = self
                .writer
                .lock()
                .map_err(|_| InklogError::IoError(io::Error::other("Lock poisoned")))?;
            self.write_record(&mut *writer, &masked_record, use_color)
                .map_err(InklogError::IoError)?;
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), InklogError> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| InklogError::IoError(io::Error::other("Lock poisoned")))?;
        writer.flush().map_err(InklogError::IoError)
    }

    fn is_healthy(&self) -> bool {
        true
    }

    fn shutdown(&mut self) -> Result<(), InklogError> {
        self.flush()
    }
}

impl Clone for ConsoleSink {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            // Clone shares the same writer (Arc ensures reference counting)
            writer: Arc::clone(&self.writer),
            template: self.template.clone(),
            masker: DataMasker::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConsoleSinkConfig;
    use serial_test::serial;
    use std::env;

    fn get_sink() -> ConsoleSink {
        ConsoleSink::new(
            ConsoleSinkConfig {
                enabled: true,
                colored: true,
                ..Default::default()
            },
            LogTemplate::default(),
        )
    }

    #[test]
    #[serial]
    fn test_no_color_env() {
        let sink = get_sink();
        env::set_var("NO_COLOR", "1");
        assert!(!sink.should_colorize(false));
        env::remove_var("NO_COLOR");
    }

    #[test]
    #[serial]
    fn test_force_color_env() {
        let sink = get_sink();
        // Remove NO_COLOR to ensure deterministic test result
        env::remove_var("NO_COLOR");
        env::set_var("CLICOLOR_FORCE", "1");
        assert!(sink.should_colorize(false));
        env::remove_var("CLICOLOR_FORCE");
    }

    #[test]
    #[serial]
    fn test_term_dumb() {
        let sink = get_sink();
        // Remove NO_COLOR to ensure deterministic test result
        env::remove_var("NO_COLOR");
        env::set_var("TERM", "dumb");
        // Ensure no other conflicting envs
        env::remove_var("CLICOLOR_FORCE");
        assert!(!sink.should_colorize(false));
        env::remove_var("TERM");
    }

    #[test]
    #[serial]
    fn test_config_disabled() {
        let mut sink = get_sink();
        // Remove NO_COLOR to ensure deterministic test result
        env::remove_var("NO_COLOR");
        sink.config.colored = false;
        env::set_var("CLICOLOR_FORCE", "1"); // Config should override force?
                                             // My logic: if !config.colored return false.
        assert!(!sink.should_colorize(false));
        env::remove_var("CLICOLOR_FORCE");
    }

    #[test]
    fn test_console_sink_new() {
        let config = ConsoleSinkConfig {
            enabled: true,
            colored: true,
            ..Default::default()
        };
        let template = LogTemplate::default();
        let sink = ConsoleSink::new(config, template);
        assert!(sink.config.enabled);
    }

    #[test]
    fn test_console_sink_disabled() {
        let config = ConsoleSinkConfig {
            enabled: false,
            colored: true,
            ..Default::default()
        };
        let template = LogTemplate::default();
        let sink = ConsoleSink::new(config, template);
        assert!(!sink.config.enabled);
    }

    #[test]
    #[serial]
    fn test_should_colorize_defaults() {
        env::remove_var("CLICOLOR_FORCE");
        env::remove_var("TERM");
        env::set_var("NO_COLOR", "1");
        let config = ConsoleSinkConfig {
            enabled: true,
            colored: true,
            ..Default::default()
        };
        let template = LogTemplate::default();
        let sink = ConsoleSink::new(config, template);
        let result = sink.should_colorize(false);
        assert!(
            !result,
            "should_colorize should return false when NO_COLOR is set"
        );
        env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_should_colorize_when_allowed() {
        let sink = get_sink();
        let colored = sink.apply_color("test message", "ERROR");
        assert!(colored.contains("test message"));
    }

    #[test]
    fn test_apply_color_info() {
        let sink = get_sink();
        let colored = sink.apply_color("test message", "INFO");
        assert!(colored.contains("test message"));
    }

    #[test]
    fn test_apply_color_unknown() {
        let sink = get_sink();
        let colored = sink.apply_color("test message", "UNKNOWN");
        assert!(colored.contains("test message"));
    }
}
