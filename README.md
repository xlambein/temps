# `temps`, a Minimalist CLI Time Tracker

## Installation

Clone the repo and install with `cargo install`:

```sh
git clone https://github.com/xlambein/temps.git
cd temps
cargo install --path .
```

If you don't have `cargo`, you can [install it here](https://doc.rust-lang.org/cargo/getting-started/installation.html).

## Usage

Start tracking:

```sh
$ temps start "world domination"
Started 'world domination'.
```

Starting a new timer stops the previous one:

```sh
$ temps start "studying category theory"
Stopped 'world domination'.
Started 'studying category theory'.
```

Stop tracking:

```sh
$ temps stop
Stopped 'studying category theory'.
```

Summary of time tracked (default behaviour if no subcommand is passed):

```sh
$ temps summary
Summary for today (Sep 22)

Project                     Time  
------------------------  ------  
studying category theory      9m  
world domination          4h 24m  

TOTAL                     4h 33m  
------------------------  ------  
Project                     Time

Ongoing: word domination (1h 17m)
```

Use `temps summary --weekly` and `temps summary --full` for weekly and full summary.

Cancel a timer (deletes the entry):

```sh
$ temps start "learning javascript"
Started 'learning javascript'.
$ temps cancel
Cancelled 'learning javascript' (started at 2021-09-16T16:41:05+02:00).
```

Start tracking from a specific date/time (useful to "undo" a `cancel` command):

```sh
$ # RFC3339 datetime:
$ temps start "learning rust" --from 2021-09-16T16:41:05+02:00
Started 'learning rust'.

$ # Time only (infers date is today):
$ temps start "learning rust" --from 16:41
Started 'learning rust'.
```

Display a graph of the time spent on a given day (defaults to today):

``` sh
$ temps viz
▁▁▁▁▁▁
10:00 
      ▀▀▀▀▀▀▀▀ studying category theory
      
▁▁▁▁▁▁████████ world domination
12:00 
      
      
▁▁▁▁▁▁
14:00 
      ▄▄▄▄▄▄▄▄ learning javascript
      
▁▁▁▁▁▁
16:00 
      ████████ learning javascript / learning rust
      ████████
▁▁▁▁▁▁████████
18:00 
$ # Also works with:
$ temps viz yesterday
$ temps viz "5 days ago"
$ temps viz 2021-08-10
```

Edit the raw data with your `$EDITOR`:

``` sh
$ temps edit
```

Tracking data is stored in `~/temps.tsv`.  This location can be changed by setting the environment variable `TEMPS_FILE`, or by passing `--temps-file [PATH]` to `temps`.

By default, the day is assumed to start at midnight of your local timezone.  To change that, you can set the `TEMPS_MIDNIGHT_OFFSET` environment variable, or pass the `--midnight-offset` option.  It expects a duration of the form `HH:MM` or `HH:MM:SS`.

## Autocompletions

Autocompletions for common shells are provided courtesy of [`clap_complete`](https://crates.io/crates/clap_complete).  Just pipe the output of the following command into the appropriate file for your shell.

``` sh
$ temps --generate-completions <SHELL>
```

For example, if you're using Fish Shell:

``` sh
$ temps --generate-completions fish > ~/.config/fish/completions/temps.fish
```
