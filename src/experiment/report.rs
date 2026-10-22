//! Markdown reports in the fixed shape of docs/03-experiments.md §8.

use std::fmt::Write;

#[derive(Clone, Debug)]
pub enum Verdict {
    Supported,
    Refuted,
    Inconclusive,
}

impl Verdict {
    pub fn label(&self) -> &'static str {
        match self {
            Verdict::Supported => "supported",
            Verdict::Refuted => "refuted",
            Verdict::Inconclusive => "inconclusive",
        }
    }
}

#[derive(Default)]
pub struct Report {
    pub title: String,
    pub configuration: Vec<(String, String)>,
    pub observations: String,
    pub verdicts: Vec<(String, Verdict, String)>,
    pub interpretation: String,
}

pub const NOT_CLAIMED: &str = "No claim is made about subjective experience. \
No claim is made about transfer of experience under snapshot or fork. \
No claim extends beyond the tested scale, environment families and seeds. \
Two authored policy fragments exist (F1: move toward a remembered resource, \
priced by the world model; F2: seek the least-known neighbor) and are shared \
with the baselines; nothing in these results credits them to the organism.";

impl Report {
    pub fn new(title: &str) -> Report {
        Report { title: title.to_string(), ..Report::default() }
    }

    pub fn config(&mut self, key: &str, value: impl ToString) {
        self.configuration.push((key.to_string(), value.to_string()));
    }

    pub fn verdict(&mut self, hypothesis: &str, v: Verdict, detail: impl ToString) {
        self.verdicts.push((hypothesis.to_string(), v, detail.to_string()));
    }

    pub fn markdown(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# {}\n", self.title);
        let _ = writeln!(s, "## 1. Configuration\n");
        for (k, v) in &self.configuration {
            let _ = writeln!(s, "- {k}: {v}");
        }
        let _ = writeln!(s, "\n## 2. Observations\n");
        s.push_str(&self.observations);
        let _ = writeln!(s, "\n## 3. Hypothesis verdicts\n");
        for (h, v, d) in &self.verdicts {
            let _ = writeln!(s, "- **{h}**: {} — {d}", v.label());
        }
        let _ = writeln!(s, "\n## 4. Interpretation\n");
        s.push_str(&self.interpretation);
        let _ = writeln!(s, "\n\n## 5. Not claimed\n\n{NOT_CLAIMED}");
        s
    }
}

/// Markdown table helper.
pub struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(header: &[&str]) -> Table {
        Table { header: header.iter().map(|h| h.to_string()).collect(), rows: Vec::new() }
    }

    pub fn row(&mut self, cells: Vec<String>) {
        self.rows.push(cells);
    }

    pub fn markdown(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "| {} |", self.header.join(" | "));
        let _ = writeln!(s, "|{}|", self.header.iter().map(|_| "---").collect::<Vec<_>>().join("|"));
        for r in &self.rows {
            let _ = writeln!(s, "| {} |", r.join(" | "));
        }
        s
    }
}

pub fn f3(x: f64) -> String {
    if x.is_nan() { "n/a".into() } else { format!("{x:.3}") }
}

pub fn opt3(x: Option<f64>) -> String {
    x.map(f3).unwrap_or_else(|| "n/a".into())
}
