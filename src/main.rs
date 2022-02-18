use std::convert::TryInto;
use std::env;
use std::process::Command;
use std::{collections::BTreeMap, fmt::Write, path::Path};

use anyhow::{bail, Context, Result};
use clap::{IntoApp, Parser};
use clap_complete::{generate, Shell};
use csv::{ReaderBuilder, WriterBuilder};
use serde::{Deserialize, Serialize};
use time::ext::NumericalDuration;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{Date, Duration, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

mod table;

use table::{Alignment, Table};

const FULL_BLOCK: char = '█';
const UPPER_HALF_BLOCK: char = '▀';
const LOWER_HALF_BLOCK: char = '▄';
const LOWER_BORDER: char = '▁';

trait TruncateSubseconds {
    fn truncate_subseconds(self) -> Self;
}

impl TruncateSubseconds for Time {
    fn truncate_subseconds(self) -> Self {
        let (h, m, s) = self.as_hms();
        Time::from_hms(h, m, s).unwrap()
    }
}

impl TruncateSubseconds for PrimitiveDateTime {
    fn truncate_subseconds(self) -> Self {
        self.replace_time(self.time().truncate_subseconds())
    }
}

impl TruncateSubseconds for OffsetDateTime {
    fn truncate_subseconds(self) -> Self {
        self.replace_time(self.time().truncate_subseconds())
    }
}

/// Parse a date and time, possibly inferring the date.
///
/// Expects either an RFC3339-formatted date/time, or a time with format
/// `HH:MM:SS` or `HH:MM` (in which case the date is set to the current date).
fn parse_datetime(src: &str) -> Result<OffsetDateTime> {
    PrimitiveDateTime::parse(src, &Rfc3339)
        .map_err(anyhow::Error::from)
        .and_then(|dt| Ok(dt.assume_offset(UtcOffset::current_local_offset()?)))
        .or_else(|_| {
            // Try to parse either HH:MM:SS or HH:MM:SS
            let time = Time::parse(src, &format_description!("[hour]:[minute]:[second]"))
                .or_else(|_| Time::parse(src, &format_description!("[hour]:[minute]")))?;
            // Extend time with current date
            OffsetDateTime::now_local()
                .map_err(anyhow::Error::from)
                .map(|dt| dt.replace_time(time))
        })
        .context("Could not parse start date")
}

/// Parse a duration.
///
/// Expects a duration with format `HH:MM:SS` or `HH:MM`.
fn parse_duration(src: &str) -> Result<Duration> {
    // Try to parse a time
    Time::parse(src, &format_description!("[hour]:[minute]:[second]"))
        .or_else(|_| Time::parse(src, &format_description!("[hour]:[minute]")))
        .context("Could not parse duration")
        // Convert it to a duration by subtracting midnight
        .map(|time| time - Time::MIDNIGHT)
}

/// Parse a (possibly relative) date.
///
/// Expects either `YYYY-mm-dd`, `today`, `yesterday`, or `N days ago` where `N`
/// is a positive integer.
fn parse_date(src: &str) -> Result<Date> {
    // Try to parse a YYYY-mm-dd date
    Date::parse(src, &format_description!("[year]-[month]-[day]"))
        .map_err(anyhow::Error::from)
        // Try to parse a literal 'today', 'yesterday' or 'N days ago'
        .or_else(|err| {
            if src == "today" {
                Ok(OffsetDateTime::now_local()?.date())
            } else if src == "yesterday" {
                Ok(OffsetDateTime::now_local()?.date() - 1.days())
            } else if let Some((days, s)) = src.split_once(" ") {
                if s.trim() == "days ago" {
                    if let Ok(days) = days.parse::<i64>() {
                        return Ok(OffsetDateTime::now_local()?.date() - days.days());
                    }
                }
                Err(err)
            } else {
                Err(err)
            }
        })
        .context("Could not parse date")
}

#[derive(Parser, Debug)]
#[clap(about = "Simple time tracker.", version, author)]
struct Args {
    #[clap(subcommand)]
    subcommand: Option<Subcommand>,
    #[clap(
        long,
        env,
        default_value = "~/temps.tsv",
        help = "Path for the tracking data"
    )]
    temps_file: String,
    #[clap(
        long,
        env = "TEMPS_MIDNIGHT_OFFSET",
        parse(try_from_str = parse_duration),
        default_value = "00:00",
        help = "Time at which we consider the current day to have ended"
        // It's not necessarily midnight because sometimes we make poor choices
    )]
    midnight_offset: Duration,
    #[clap(
        long,
        value_name = "SHELL",
        help = "Generate completions for a given shell"
    )]
    generate_completions: Option<Shell>,
}

#[derive(Parser, Debug)]
enum Subcommand {
    #[clap(
        about = "Display a summary of the time tracked per project",
        display_order = 0
    )]
    Summary {
        #[clap(short, long, conflicts_with_all = &["weekly", "daily"], display_order=0, help = "Time tracked forever")]
        full: bool,
        #[clap(short, long, conflicts_with_all = &["full", "daily"], display_order=1, help = "Time tracked in the past week")]
        weekly: bool,
        #[clap(short, long, conflicts_with_all = &["full", "weekly"], display_order=2, help = "Time tracked today (default)")]
        daily: bool,
    },
    #[clap(about = "Start new timer", display_order = 1)]
    Start {
        #[clap(help = "Project name (defaults to last project)")]
        project: Option<String>,
        #[clap(long, short, parse(try_from_str = parse_datetime), help = "Start date (defaults to now)")]
        from: Option<OffsetDateTime>,
    },
    #[clap(about = "Stop ongoing timer", display_order = 2)]
    Stop {
        #[clap(long, short, parse(try_from_str = parse_datetime), help = "Stop date (defaults to now)")]
        at: Option<OffsetDateTime>,
    },
    #[clap(about = "Cancel ongoing timer", display_order = 3)]
    Cancel,
    #[clap(about = "List raw data", display_order = 4)]
    List,
    #[clap(about = "Edit raw data with default editor", display_order = 5)]
    Edit,
    #[clap(
        about = "Visualize time spent on a given day",
        display_order = 5,
        name = "viz"
    )]
    Visualize {
        #[clap(parse(try_from_str = parse_date), help = "Date (defaults to today)")]
        date: Option<Date>,
    },
}

impl Default for Subcommand {
    fn default() -> Self {
        Subcommand::Summary {
            full: false,
            weekly: false,
            daily: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// A time-tracking entry associated with a project.
struct Entry {
    project: String,
    #[serde(with = "time::serde::rfc3339")]
    start: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    end: Option<OffsetDateTime>,
}

impl Entry {
    /// Start a new entry from the current date/time.
    fn start(project: String) -> Self {
        Self::start_from(
            project,
            OffsetDateTime::now_local().expect("Could not determine local datetime"),
        )
    }

    /// Start a new entry from a specific date/time.
    ///
    /// Panics if the start time is in the future.
    fn start_from(project: String, start: OffsetDateTime) -> Self {
        if start > OffsetDateTime::now_local().expect("Could not determine local datetime") {
            panic!("Start date is in the future");
        }
        Self {
            project,
            start: start.truncate_subseconds(),
            end: None,
        }
    }

    /// Stop the entry at the current date/time.
    fn stop(&mut self) {
        self.stop_at(OffsetDateTime::now_local().expect("Could not determine local datetime"))
    }

    /// Stop the entry at a specific date/time.
    ///
    /// Panics if the end time is in the future, or is before the start time.
    fn stop_at(&mut self, end: OffsetDateTime) {
        if end > OffsetDateTime::now_local().expect("Could not determine local datetime") {
            panic!("End date is in the future");
        }
        if end < self.start {
            panic!("End date is before start date");
        }
        self.end = Some(end.truncate_subseconds());
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
    let args = Args::parse();

    if let Some(shell) = args.generate_completions {
        // Generate completions then exit
        let mut app = Args::into_app();
        let bin_name = app.get_name().to_string();

        if shell == Shell::Fish {
            // For fish shell, never complete on file names
            println!("complete -c {} -f", bin_name);
        }

        generate(shell, &mut app, bin_name, &mut std::io::stdout());

        return Ok(());
    }

    let path = Path::new(&args.temps_file);

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

    match args.subcommand.unwrap_or_default() {
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

        Subcommand::Stop { at } => {
            let last = entries.last_mut().context("No previous entry exists")?;

            if !last.is_ongoing() {
                bail!("No ongoing entry");
            }

            if let Some(at) = at {
                last.stop_at(at);
            } else {
                last.stop();
            }
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
                entry.start.format(&Rfc3339)?
            );

            write_back(path, &entries)?;
        }

        Subcommand::List => {
            let mut table = Table::new(["Project", "Start", "End"]);
            for entry in &entries {
                table.row([
                    entry.project.clone(),
                    entry.start.format(&Rfc3339)?,
                    entry
                        .end
                        .as_ref()
                        .map(|dt| dt.format(&Rfc3339))
                        .transpose()?
                        .unwrap_or_else(String::new),
                ]);
            }
            print!("{}", table);
        }

        Subcommand::Summary { full: true, .. } => {
            // BTreeMap instead of HashMap so the keys are sorted :>
            let mut summary = BTreeMap::new();

            let now = OffsetDateTime::now_local()?;

            // Collect total time on each project
            for entry in &entries {
                let total = summary
                    .entry(entry.project.clone())
                    .or_insert(Duration::ZERO);
                *total += entry.end.unwrap_or(now) - entry.start;
            }

            // Display summary as a table
            let mut table = Table::new(["Project", "Time"]);
            table.align([Alignment::Left, Alignment::Right]);
            for (project, duration) in summary {
                table.row([project, duration_to_string(duration)?]);
            }
            print!("{}", table);

            if let Some(last) = &entries.last() {
                if last.is_ongoing() {
                    println!();
                    println!(
                        "Ongoing: {} ({})",
                        last.project,
                        duration_to_string(now - last.start)?
                    );
                }
            }
        }

        // Weekly
        Subcommand::Summary { weekly: true, .. } => {
            // BTreeMap instead of HashMap so the keys are sorted :>
            let mut summary = BTreeMap::<String, [Duration; 7]>::new();
            let mut daily_total = [Duration::ZERO; 7];

            let now = OffsetDateTime::now_local()?;
            let today = now.date();

            // Collect daily total time on each project
            for entry in &entries {
                let start = entry.start - args.midnight_offset;
                let end = entry.end.unwrap_or(now) - args.midnight_offset;

                // Iterate over every day between `start` and `end`.
                // `min(6)` ensures that we don't consider start dates beyond one week
                for delta in (today - end.date()).whole_days() as usize
                    ..=(today - start.date()).whole_days().min(6) as usize
                {
                    let totals = summary.entry(entry.project.clone()).or_default();

                    // Duration is min(end, today - delta + 1 day) - max(start, today - delta)
                    let duration = end
                        .min(now.replace_time(Time::MIDNIGHT) - (delta as i64 - 1).days())
                        - start.max(now.replace_time(Time::MIDNIGHT) - (delta as i64).days());
                    totals[delta] += duration;
                    daily_total[delta] += duration;
                }
            }

            println!("Summary for the past week");
            println!();

            fn week_row<T: std::fmt::Debug>(
                first: impl Into<T>,
                rest: impl IntoIterator<Item = T>,
            ) -> [T; 8] {
                let mut row = vec![first.into()];
                row.extend(rest.into_iter());
                row.try_into().unwrap()
            }

            // Display summary as a table
            let headers = week_row(
                "Project".to_owned(),
                (0..7)
                    .rev()
                    .map(|i| today - Duration::days(i))
                    .map(|d| d.format(&format_description!("[weekday]")))
                    .collect::<Result<Vec<_>, _>>()?,
            );
            let alignments = week_row(Alignment::Left, vec![Alignment::Right; 7]);

            let mut table = Table::<8>::new(headers);
            table.align(alignments);
            for (project, durations) in summary {
                let row = week_row(
                    project,
                    durations
                        .into_iter()
                        .rev()
                        .map(|d| duration_to_string(d).expect("could not format duration")),
                );
                table.row(row);
            }

            table.row(vec![String::new(); 8].try_into().unwrap());

            let row = week_row(
                "TOTAL".to_owned(),
                daily_total
                    .into_iter()
                    .rev()
                    .map(|d| duration_to_string(d).expect("could not format duration")),
            );
            table.row(row);

            print!("{}", table);

            println!();
            println!(
                "Weekly total: {}",
                duration_to_string(daily_total.into_iter().sum())?
            );

            if let Some(last) = &entries.last() {
                if last.is_ongoing() {
                    println!();
                    println!(
                        "Ongoing: {} ({})",
                        last.project,
                        duration_to_string(now - last.start)?
                    );
                }
            }
        }

        // Daily summary
        Subcommand::Summary { .. } => {
            // BTreeMap instead of HashMap so the keys are sorted :>
            let mut summary = BTreeMap::new();
            let mut daily_total = Duration::ZERO;

            let now = OffsetDateTime::now_local()?;
            let today = now.date();

            // Collect total time on each project
            for entry in &entries {
                // Actual start time is max(today at midnight, start),
                // in case the entry started the day before
                let start =
                    (entry.start - args.midnight_offset).max(now.replace_time(Time::MIDNIGHT));
                let end = entry.end.unwrap_or(now) - args.midnight_offset;

                if end.date() == today {
                    let total = summary.entry(entry.project.clone()).or_default();

                    let duration = end - start;
                    *total += duration;
                    daily_total += duration;
                }
            }

            println!(
                "Summary for today ({})",
                today.format(&format_description!(
                    "[month repr:short] [day padding:zero]"
                ))?
            );
            println!();

            // Display summary as a table
            let mut table = Table::new(["Project", "Time"]);
            table.align([Alignment::Left, Alignment::Right]);
            for (project, duration) in summary {
                table.row([project, duration_to_string(duration)?]);
            }
            table.row(["", ""]);
            table.row(["TOTAL".to_owned(), duration_to_string(daily_total)?]);
            print!("{}", table);

            if let Some(last) = &entries.last() {
                if last.is_ongoing() {
                    println!();
                    println!(
                        "Ongoing: {} ({})",
                        last.project,
                        duration_to_string(now - last.start)?
                    );
                }
            }
        }

        Subcommand::Edit => {
            let editor = env::var("EDITOR")
                .expect("no default editor, set the $EDITOR environment variable");
            Command::new(&editor)
                .arg(&args.temps_file)
                .status()
                .unwrap_or_else(|_| panic!("could not run editor '{}'", editor));
        }

        Subcommand::Visualize { date } => {
            // TODO a possibly more elegant way of doing all this is to use a sort of
            //   hash map or something, which can be queried for each slot.  Then, we
            //   iterate from the first slot we care about (i.e., slightly before the
            //   first project slot), and query two slots at a time, displaying them
            //   if there's a project.  This would also make it easier to scale this to
            //   multiple projects.

            let now = OffsetDateTime::now_local()?;
            let today = now.date();

            let date = date
                .unwrap_or(today)
                .with_time(Time::MIDNIGHT)
                .assume_offset(now.offset());
            let next_date = date + Duration::days(1);

            let mut slots = vec![];
            let mut previous_end = None;

            for entry in &entries {
                let start = entry.start;
                let end = entry.end.unwrap_or(now);

                // Does the entry overlap with today?
                if start < next_date && end >= date {
                    // Convert start/end to quarter-hours
                    let s = ((start.max(date).time() - Time::MIDNIGHT).whole_minutes() as f32 / 15.)
                        .round() as i64;
                    let e = ((end.min(next_date).time() - Time::MIDNIGHT).whole_minutes() as f32
                        / 15.)
                        .round() as i64;
                    if s == e {
                        // Skip very short slots
                        continue;
                    }

                    // Prepend empty slots before the first project slot
                    // We round at a half hour, that way the time is displayed properly
                    if previous_end.is_none() {
                        previous_end = Some((s / 8) * 8 - 2);
                    }

                    // Fill with empty slots since last entry
                    if let Some(previous_end) = previous_end {
                        slots.extend((previous_end..s).into_iter().map(|i| (i, None)));
                    }
                    previous_end = Some(e);

                    // Fill with project slots for the duration of the entry
                    slots.extend((s..e).into_iter().map(|i| (i, Some(&entry.project))));
                }
            }

            // Add one or two empty slots at the end if we're close to a two-hour mark
            // This makes the display slightly prettier :>
            if let Some((last, _)) = slots.last() {
                let last = *last; // Otherwise rustc says we can't mutate `slots` :<
                if last % 8 >= 6 {
                    slots.extend(
                        ((last + 1)..=(last / 8 + 1) * 8)
                            .into_iter()
                            .map(|i| (i, None)),
                    );
                }
            }

            let mut previous_project = None;
            let times_width = 6;
            let width = 8;
            for chunks in slots.chunks(2) {
                let i = chunks[0].0;
                // Display the time every two hours
                if i % 8 == 0 {
                    print!(
                        "{:width$} ",
                        (Time::MIDNIGHT + (i * 15).minutes())
                            .format(&format_description!("[hour]:[minute]"))?,
                        width = times_width - 1
                    );
                } else if i % 8 == 6 {
                    print!("{}", LOWER_BORDER.to_string().repeat(times_width));
                } else {
                    print!("{}", " ".repeat(times_width));
                }

                // Display the current two slots with half-blocks
                match chunks {
                    &[(_, None), (_, None)] | &[(_, None)] => {
                        previous_project = None;
                    }
                    &[(_, None), (_, Some(p1))] => {
                        print!("{}", LOWER_HALF_BLOCK.to_string().repeat(width));
                        print!(" {}", p1);
                        previous_project = Some(p1);
                    }
                    &[(_, Some(p0)), (_, None)] | &[(_, Some(p0))] => {
                        print!("{}", UPPER_HALF_BLOCK.to_string().repeat(width));
                        if previous_project != Some(p0) {
                            print!(" {}", p0);
                        }
                        previous_project = None;
                    }
                    &[(_, Some(p0)), (_, Some(p1))] => {
                        print!("{}", FULL_BLOCK.to_string().repeat(width));
                        if previous_project != Some(p0) {
                            print!(" {}", p0);
                            if p0 != p1 {
                                print!(" / {}", p1);
                            }
                        } else if p0 != p1 {
                            print!(" {}", p1);
                        }
                        previous_project = Some(p1);
                    }
                    _ => unreachable!(),
                }
                println!();
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
    let minutes = duration.whole_minutes();
    let hours = minutes / 60;
    let minutes = minutes % 60;

    let mut result = String::new();
    if hours > 0 {
        write!(result, "{}h {:02}m", hours, minutes)?;
    } else {
        write!(result, "{}m", minutes)?;
    }

    Ok(result)
}
