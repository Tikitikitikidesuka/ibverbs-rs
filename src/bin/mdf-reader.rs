/*!
# 💫🛠️ MDF Reader
[![Static Badge](https://img.shields.io/badge/docs-available-blue)](https://lb-rusteb-docs.docs.cern.ch/mdf_reader)

This binary is a simple tool to view MDF files.
## Example
```bash
mdf-reader /path/to/file.mdf
```

Output:
<head>
<style type="text/css">
.ansi2html-content { white-space: pre-wrap; word-wrap: break-word; }
.body_foreground { color: #AAAAAA; }
.body_background { background-color: #181818; }
.inv_foreground { color: #000000; }
.inv_background { background-color: #AAAAAA; }
.ansi1 { font-weight: bold; }
.ansi30 { color: #868686; }
.ansi32 { color: #23d18b; }
.ansi34 { color: #3b8eea; }
</style>
</head>
<body class="body_foreground " style="font-size: normal;" >
<pre class="ansi2html-content">
<span class="ansi1 ansi34">MDF Record</span> <span class="ansi30">Run</span> 328614, <span class="ansi30">Orbit</span> 459008, <span class="ansi30">Bunch</span> 524544
  <span class="ansi1 ansi30">Fragment</span> <span class="ansi1 ansi32">Odin</span> (0x10) <span class="ansi30">Version</span> 7<span class="ansi30">, Source</span> Odin-0x0000 (0x1)<span class="ansi30">, Size</span> 40 <span class="ansi30">bytes</span>
    <span class="ansi30">Event Id       </span> 3998801 (0x3D0451)
    <span class="ansi30">Event Type     </span> 4
    <span class="ansi30">Time           </span> 2025-08-28 11:52:34.122925 +00
    <span class="ansi30">Partition      </span> 0x008000
    <span class="ansi30">Step enabled?  </span> false
    <span class="ansi30">Orbit Id       </span> 1778 (0x6F2)
    <span class="ansi30">Bunch Id       </span> 1042 (0x412)
    <span class="ansi30">BunchType      </span> (true, true)
    <span class="ansi30">TCK            </span> 268439840
    <span class="ansi30">Is nzs event?  </span> false
    <span class="ansi30">Calib type     </span> 0 (0x0)
    <span class="ansi30">Trigger type   </span> 6 (0x6)
    <span class="ansi30">Tae disabled</span>
  <span class="ansi1 ansi30">Fragment</span> <span class="ansi1 ansi32">TestDet</span> (0x33) <span class="ansi30">Version</span> 1<span class="ansi30">, Source</span> Tdet-0x0000 (0x780D)<span class="ansi30">, Size</span> 75 <span class="ansi30">bytes</span>
    0000:   04f4 f4f4  f4f4 f4f4  f4f4 f4f4  f04f 4f4f   .............OOO
    0010:   4f4f 4f4f  4f4f 4f4f  4f04 f4f4  f4f4 f4f4   OOOOOOOOO.......
    0020:   f4f4 f4f4  f4f0 4f4f  4f4f 4f4f  4f4f 4f4f   ......OOOOOOOOOO
    0030:   4f4f 04f4  f4f4 f4f4  f4f4 f4f4  f4f4 f04f   OO.............O
    0040:   4f4f 4f4f  4f4f 4f4f  4f4f 4f                OOOOOOOOOOO
  <span class="ansi1 ansi30">Fragment</span> <span class="ansi1 ansi32">TestDet</span> (0x33) <span class="ansi30">Version</span> 1<span class="ansi30">, Source</span> Tdet-0x0000 (0x780F)<span class="ansi30">, Size</span> 75 <span class="ansi30">bytes</span>
    0000:   04f4 f4f4  f4f4 f4f4  f4f4 f4f4  f04f 4f4f   .............OOO
    0010:   4f4f 4f4f  4f4f 4f4f  4f04 f4f4  f4f4 f4f4   OOOOOOOOO.......
    0020:   f4f4 f4f4  f4f0 4f4f  4f4f 4f4f  4f4f 4f4f   ......OOOOOOOOOO
    0030:   4f4f 04f4  f4f4 f4f4  f4f4 f4f4  f4f4 f04f   OO.............O
    0040:   4f4f 4f4f  4f4f 4f4f  4f4f 4f                OOOOOOOOOOO
</pre>
</body>

## Usage
```text
Crate containing examples and tools utilizing the other rusteb crates.

Usage: mdf-reader [OPTIONS] <FILE>

Arguments:
  <FILE>  File to read

Options:
      --color <COLOR>  Controls whether colored output is used [default: auto] [possible values: auto, always, never]
  -h, --help           Print help
  -V, --version        Print version
```
*/

use std::{
    io::{BufWriter, stdout},
    path::PathBuf,
};

use clap::{ColorChoice, Parser};
use colored::Colorize;
use master_data_file::MdfFile;
use pretty_hex::{HexConfig, config_hex};
use std::io::Write;
use tracing::{debug, level_filters::LevelFilter, trace};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// File to read
    file: PathBuf,
    #[arg(long, default_value_t)]
    /// Controls whether colored output is used
    color: ColorChoice,
    #[arg(long, default_value_t = LevelFilter::WARN)]
    log_level: LevelFilter,
}

pub fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(args.log_level)
        .init();
    trace!("Initialized logging.");
    debug!("Using arguments: {:?}", args);

    match args.color {
        ColorChoice::Auto => (),
        ColorChoice::Always => colored::control::set_override(true),
        ColorChoice::Never => colored::control::set_override(false),
    }

    let x = MdfFile::mmap_file(&args.file)?;
    trace!("Opened file {:?} with mmap", args.file);

    let mut output = BufWriter::new(stdout());

    let config = HexConfig {
        title: false,
        ascii: true,
        width: 16,
        group: 2,
        chunk: 2,
        max_bytes: 256,
        display_offset: 0,
    };

    for rec in &x {
        write!(output, "{}", "MDF Record".bold().blue())?;
        if let Ok(rec) = rec.try_into_single_event() {
            writeln!(
                output,
                " {} {}, {} {}, {} {}",
                "Run".black(),
                rec.specific_header().run_number,
                "Orbit".black(),
                rec.specific_header().orbit_count,
                "Bunch".black(),
                rec.specific_header().bunch_identifier
            )?;

            for frag in rec.fragments() {
                frag.pretty_print(&mut output, 2)?;
            }
        } else {
            writeln!(
                output,
                " {} {}",
                "Header type".black(),
                rec.specific_header_type()
            )?;
            writeln!(
                output,
                "  {}",
                config_hex(&rec.body_bytes(), config).replace('\n', "\n  ")
            )?;
        }
    }

    output.flush()?;
    Ok(())
}
