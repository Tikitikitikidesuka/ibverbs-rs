use std::{
    io::{BufWriter, ErrorKind, stdout},
    path::PathBuf,
};

use anyhow::bail;
use clap::{ColorChoice, Parser};
use colored::Colorize;
use ebutils::Fragment;
use multi_event_packet::MultiEventPacketOwned;
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
    #[arg(long, default_value_t = false)]
    expand_mfps: bool,
}

fn main() -> anyhow::Result<()> {
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

    match run(&args) {
        Ok(()) => (),
        Err(e) if e.kind() == ErrorKind::BrokenPipe => (),
        Err(e) => bail!(e),
    }

    Ok(())
}

fn run(args: &Args) -> std::io::Result<()> {
    let mep = MultiEventPacketOwned::mmap_file(&args.file)?;
    trace!("Opened file {:?} with mmap", args.file);

    let mut output = BufWriter::new(stdout());

    writeln!(
        output,
        "{} {} {:?}{} {} {} {} {}",
        "MEP".bold().green(),
        "for events with ID".black(),
        mep.event_id_range(),
        ",",
        mep.num_mfps(),
        "MFPs, each with".black(),
        mep.get_mfp(0).unwrap().fragment_count(),
        "fragments".black(),
    )?;

    for mfp in mep.mfp_iter() {
        writeln!(
            output,
            "  {} {} {}{} {}",
            "MFP".bold().blue(),
            "Source ID".black(),
            mfp.source_id(),
            ", Fragment Version".black(),
            mfp.fragment_version()
        )?;

        let fragments: &mut dyn Iterator<Item = Fragment> = if args.expand_mfps {
            &mut mfp.fragment_iter()
        } else {
            &mut mfp.fragment_iter().take(1)
        };

        for frag in fragments {
            frag.pretty_print(&mut output, 4)?;
        }
        if !args.expand_mfps && mfp.fragment_count() > 0 {
            writeln!(output, "    ... and {} more ...", mfp.fragment_count() - 1,)?;
        }
        output.flush()?;
    }

    output.flush()?;
    Ok(())
}
