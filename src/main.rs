use std::{collections::BTreeMap, fmt::Write, path::Path};

use anyhow::{bail, Context, Result};
use chrono::{prelude::*, Duration};
use csv::{ReaderBuilder, WriterBuilder};
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

mod table;

use table::{Alignment, Table};

/// Parse a start date.
///
/// Expects either an RFC3339-formatted date/time, or a time with format
/// `HH:MM:SS` or `HH:MM` (in which case the date is set to the current date).
fn parse_start_date(src: &str) -> Result<DateTime<Local>> {
    DateTime::parse_from_rfc3339(src)
        .map(|dt| dt.with_timezone(&Local))
        .or_else(|_| {
            Local::now()
                .date()
                .and_time(
                    NaiveTime::parse_from_str(src, "%H:%M:%S")
                        .or_else(|_| NaiveTime::parse_from_str(src, "%H:%M"))?,
                )
                .context("Could not parse start date")
        })
}

#[derive(StructOpt, Debug)]
#[structopt(about = "Simple time tracker.")]
struct Opt {
    #[structopt(subcommand)]
    subcommand: Option<Subcommand>,
    #[structopt(
        long,
        env,
        default_value = "~/temps.tsv",
        help = "Path for the tracking data"
    )]
    temps_file: String,
}

#[derive(StructOpt, Debug)]
enum Subcommand {
    #[structopt(about = "Display total time tracked per project", display_order = 0)]
    Summary,
    #[structopt(about = "Start new timer", display_order = 1)]
    Start {
        #[structopt(help = "Project name (defaults to last project)")]
        project: Option<String>,
        #[structopt(long, short, parse(try_from_str = parse_start_date), help = "Start date (defaults to now)")]
        from: Option<DateTime<Local>>,
    },
    #[structopt(about = "Stop ongoing timer", display_order = 2)]
    Stop,
    #[structopt(about = "Cancel ongoing timer", display_order = 3)]
    Cancel,
    #[structopt(about = "List raw data", display_order = 4)]
    List,
}

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    project: String,
    start: DateTime<Local>,
    end: Option<DateTime<Local>>,
}

impl Entry {
    /// A time-tracking entry associated with a project.

    /// Start a new entry from the current date/time.
    fn start(project: String) -> Self {
        Self::start_from(project, Local::now())
    }

    /// Start a new entry from a specific date/time.
    ///
    /// Panics if the start time is in the future.
    fn start_from(project: String, start: DateTime<Local>) -> Self {
        if start > Local::now() {
            panic!("Start date is in the future");
        }
        Self {
            project,
            start: start.trunc_subsecs(0),
            end: None,
        }
    }

    /// Stop the entry at the current date/time.
    fn stop(&mut self) {
        self.end = Some(Local::now().trunc_subsecs(0))
    }

    /// Check whether the entry is still tracking time.
    fn is_ongoing(&self) -> bool {
        self.end.is_none()
    }
}

/// Write entries back to a time tracking file
fn write_back<P: AsRef<Path>>(path: P, entries: &[Entry]) -> Result<()> {
    let mut writer = WriterBuilder::new()
        .delimiter(b'\t')
        .from_path(path)
        .context("Could not open tracking file")?;
    for entry in entries {
        writer
            .serialize(entry)
            .context("Could not write entry to file")?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    let path = Path::new(&opt.temps_file);

    // Read entry file if it exists
    let mut entries = if path.exists() {
        ReaderBuilder::new()
            .delimiter(b'\t')
            .from_path(path)
            .context("Could not open tracking file")?
            .into_deserialize()
            .collect::<Result<Vec<Entry>, csv::Error>>()
            .context("Could not read entries")?
    } else {
        vec![]
    };

    match opt.subcommand.unwrap_or(Subcommand::Summary) {
        Subcommand::Start { project, from } => {
            // Stop previous entry if it's still ongoing
            if let Some(last) = entries.last_mut() {
                if last.is_ongoing() {
                    last.stop();
                    eprintln!("Stopped '{}'.", last.project);
                }
            }

            // Use previous project as default
            let project = project
                .or_else(|| entries.last().map(|e| e.project.clone()))
                .context("Cannot infer project name, please specify")?;

            let entry = if let Some(from) = from {
                Entry::start_from(project, from)
            } else {
                Entry::start(project)
            };

            eprintln!("Started '{}'.", entry.project);
            entries.push(entry);

            write_back(path, &entries)?;
        }

        Subcommand::Stop => {
            let last = entries.last_mut().context("No previous entry exists")?;

            if !last.is_ongoing() {
                bail!("No ongoing entry");
            }

            last.stop();
            eprintln!("Stopped '{}'.", last.project);

            write_back(path, &entries)?;
        }

        Subcommand::Cancel => {
            if !entries
                .last()
                .context("No previous entry exists")?
                .is_ongoing()
            {
                bail!("No ongoing entry");
            }

            let entry = entries.pop().unwrap(); // Unwrap ok because we know there's at least one entry

            eprintln!(
                "Cancelled '{}' (started at {}).",
                entry.project,
                entry.start.to_rfc3339()
            );

            write_back(path, &entries)?;
        }

        Subcommand::List => {
            let mut table = Table::new(["Project", "Start", "End"]);
            for entry in &entries {
                table.row([
                    entry.project.clone(),
                    entry.start.to_rfc3339(),
                    entry
                        .end
                        .as_ref()
                        .map(DateTime::to_rfc3339)
                        .unwrap_or_else(String::new),
                ]);
            }
            print!("{}", table);
        }

        Subcommand::Summary => {
            // BTreeMap instead of HashMap so the keys are sorted :>
            let mut summary = BTreeMap::new();

            // Collect total time on each project
            for entry in &entries {
                let total = summary
                    .entry(entry.project.clone())
                    .or_insert_with(Duration::zero);
                *total = *total + (entry.end.unwrap_or_else(Local::now) - entry.start);
            }

            // Display summary as a table
            let mut table = Table::new(["Project", "Hours"]);
            table.align([Alignment::Left, Alignment::Right]);
            for (project, duration) in summary {
                table.row([
                    project,
                    format!("{:.2}", duration.num_minutes() as f64 / 60.),
                ]);
            }
            print!("{}", table);

            if let Some(last) = &entries.last() {
                if last.is_ongoing() {
                    println!();
                    println!(
                        "Ongoing: {} ({})",
                        last.project,
                        duration_to_string(Local::now() - last.start)?
                    );
                }
            }
        }
    }

    Ok(())
}

/// Print a duration as a human-readable string.
///
/// # Examples
///
/// ```
/// assert_eq!(
///     duration_to_string(Duration::minutes(16)).unwrap(),
///     "16m".to_owned()
/// );
/// assert_eq!(
///     duration_to_string(Duration::minutes(64)).unwrap(),
///     "1h 4m".to_owned()
/// );
/// assert_eq!(
///     duration_to_string(Duration::minutes(4000)).unwrap(),
///     "66h 40m".to_owned()
/// );
/// ```
fn duration_to_string(duration: Duration) -> Result<String, std::fmt::Error> {
    let minutes = duration.num_minutes();
    let hours = minutes / 60;
    let minutes = minutes % 60;

    let mut result = String::new();
    if hours > 0 {
        write!(result, "{}h ", hours)?;
    }
    write!(result, "{}m", minutes)?;

    Ok(result)
}
