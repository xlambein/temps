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

Project                   Hours  
------------------------  -----  
studying category theory   0.15  
world domination           4.40  

TOTAL                      4.55  
------------------------  -----  
Project                   Hours

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

Tracking data is stored in `~/temps.tsv`.  This location can be changed by setting the environment variable `TEMPS_FILE`, or by passing `--temps-file [PATH]` to `temps`.
