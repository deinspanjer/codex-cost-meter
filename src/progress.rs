use std::{
    io::Write,
    time::{Duration, Instant},
};

const RENDER_INTERVAL: Duration = Duration::from_millis(125);

pub(crate) struct Progress<'a> {
    writer: &'a mut dyn Write,
    enabled: bool,
    terminal: bool,
    indexed_files: usize,
    analyzed_rollouts: usize,
    total_rollouts: Option<usize>,
    last_render: Option<Instant>,
    last_message: Option<String>,
    rendered_terminal_line: bool,
}

impl<'a> Progress<'a> {
    pub(crate) fn new(writer: &'a mut dyn Write, force: bool, terminal: bool) -> Self {
        Self {
            writer,
            enabled: force || terminal,
            terminal,
            indexed_files: 0,
            analyzed_rollouts: 0,
            total_rollouts: None,
            last_render: None,
            last_message: None,
            rendered_terminal_line: false,
        }
    }

    pub(crate) fn start_indexing(&mut self) {
        self.render("Indexing rollout metadata: 0 files", true);
    }

    pub(crate) fn indexed_file(&mut self) {
        self.indexed_files += 1;
        let noun = if self.indexed_files == 1 {
            "file"
        } else {
            "files"
        };
        self.render(
            &format!("Indexing rollout metadata: {} {noun}", self.indexed_files),
            false,
        );
    }

    pub(crate) fn start_analysis(&mut self, total_rollouts: usize) {
        self.analyzed_rollouts = 0;
        self.total_rollouts = Some(total_rollouts);
        self.render(&self.analysis_message(), true);
    }

    pub(crate) fn analyzed_rollout(&mut self) {
        self.analyzed_rollouts += 1;
        let complete = self.total_rollouts == Some(self.analyzed_rollouts);
        self.render(&self.analysis_message(), complete);
    }

    pub(crate) fn finish(&mut self) {
        if !self.enabled {
            return;
        }
        if self.total_rollouts.is_some() {
            self.render(&self.analysis_message(), true);
        }
        if self.terminal && self.rendered_terminal_line {
            let _ = self.writer.write_all(b"\n");
            self.rendered_terminal_line = false;
        }
    }

    fn analysis_message(&self) -> String {
        match self.total_rollouts {
            Some(total) => format!("Analyzing {}/{} rollouts", self.analyzed_rollouts, total),
            None => "Analyzing rollouts".into(),
        }
    }

    fn render(&mut self, message: &str, force: bool) {
        if !self.enabled {
            return;
        }
        if self.last_message.as_deref() == Some(message) {
            return;
        }
        let now = Instant::now();
        if !force
            && self
                .last_render
                .is_some_and(|last_render| now.duration_since(last_render) < RENDER_INTERVAL)
        {
            return;
        }
        self.last_render = Some(now);
        self.last_message = Some(message.into());
        if self.terminal {
            let _ = write!(self.writer, "\r\x1b[2K{message}");
            let _ = self.writer.flush();
            self.rendered_terminal_line = true;
        } else {
            let _ = writeln!(self.writer, "{message}");
        }
    }
}

impl Drop for Progress<'_> {
    fn drop(&mut self) {
        if self.terminal && self.rendered_terminal_line {
            let _ = self.writer.write_all(b"\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Progress;

    #[test]
    fn terminal_progress_erases_each_line_and_uses_singular_file() {
        let mut output = Vec::new();
        {
            let mut progress = Progress::new(&mut output, false, true);
            progress.start_indexing();
            progress.last_render = None;
            progress.indexed_file();
            progress.start_analysis(1);
            progress.analyzed_rollout();
            progress.finish();
        }

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "\r\x1b[2KIndexing rollout metadata: 0 files\r\x1b[2KIndexing rollout metadata: 1 file\r\x1b[2KAnalyzing 0/1 rollouts\r\x1b[2KAnalyzing 1/1 rollouts\n"
        );
    }

    #[test]
    fn redirected_forced_progress_uses_complete_plain_lines() {
        let mut output = Vec::new();
        {
            let mut progress = Progress::new(&mut output, true, false);
            progress.start_indexing();
            progress.start_analysis(1);
            progress.analyzed_rollout();
            progress.finish();
        }

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "Indexing rollout metadata: 0 files\nAnalyzing 0/1 rollouts\nAnalyzing 1/1 rollouts\n"
        );
    }
}
