use clap::Parser;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use ww2ogg::{validate, CodebookLibrary, ForcePacketFormat, WemError, WwiseRiffVorbis};

/// Convert Wwise RIFF/RIFX Vorbis audio files (.wem) to standard Ogg Vorbis format.
#[derive(Parser, Debug)]
#[command(name = "ww2ogg")]
#[command(version, about, long_about = None)]
struct Args {
    /// Input .wem file
    #[arg(required = true)]
    input: PathBuf,

    /// Output .ogg file (defaults to input with .ogg extension)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Use packed_codebooks_aoTuV_603.bin codebook
    #[arg(long = "pcb-aotuv", conflicts_with_all = ["inline_codebooks", "codebook_path"])]
    aotuv_codebooks: bool,

    /// Path to custom packed codebooks file
    #[arg(long = "pcb", value_name = "FILE", conflicts_with_all = ["inline_codebooks", "aotuv_codebooks"])]
    codebook_path: Option<PathBuf>,

    /// Codebooks are inline in the data
    #[arg(long = "inline-codebooks")]
    inline_codebooks: bool,

    /// Setup packet contains full Vorbis setup (not stripped)
    #[arg(long = "full-setup")]
    full_setup: bool,

    /// Force use of modified Vorbis packets
    #[arg(long = "mod-packets", conflicts_with = "no_mod_packets")]
    mod_packets: bool,

    /// Force use of standard Vorbis packets
    #[arg(long = "no-mod-packets", conflicts_with = "mod_packets")]
    no_mod_packets: bool,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Determine output path
    let output_path = args.output.unwrap_or_else(|| {
        let mut path = args.input.clone();
        path.set_extension("ogg");
        path
    });

    // Determine packet format
    let force_packet_format = if args.mod_packets {
        ForcePacketFormat::ForceModPackets
    } else if args.no_mod_packets {
        ForcePacketFormat::ForceNoModPackets
    } else {
        ForcePacketFormat::NoForce
    };

    // Check if user specified a codebook explicitly
    let explicit_codebook =
        args.inline_codebooks || args.aotuv_codebooks || args.codebook_path.is_some();

    let result = if explicit_codebook {
        // User specified a codebook, use it directly
        let codebooks = if args.inline_codebooks {
            CodebookLibrary::empty()
        } else if let Some(ref path) = args.codebook_path {
            CodebookLibrary::from_file(path)?
        } else {
            CodebookLibrary::aotuv_codebooks()?
        };

        convert_file(
            &args.input,
            codebooks,
            args.inline_codebooks,
            args.full_setup,
            force_packet_format,
        )
    } else {
        // Auto-detect: try default codebook first, then aoTuV
        try_convert_with_auto_detection(
            &args.input,
            args.inline_codebooks,
            args.full_setup,
            force_packet_format,
        )
    };

    match result {
        Ok(data) => {
            // Write to output file
            let mut output = BufWriter::new(File::create(&output_path)?);
            std::io::Write::write_all(&mut output, &data)?;
            println!(
                "Converted {} -> {}",
                args.input.display(),
                output_path.display()
            );
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

fn convert_file(
    input_path: &PathBuf,
    codebooks: CodebookLibrary,
    inline_codebooks: bool,
    full_setup: bool,
    force_packet_format: ForcePacketFormat,
) -> Result<Vec<u8>, WemError> {
    let input = File::open(input_path)?;

    let mut converter = WwiseRiffVorbis::with_options(
        input,
        codebooks,
        inline_codebooks,
        full_setup,
        force_packet_format,
    )?;

    // Convert to memory for validation
    let mut output = Vec::new();
    converter.generate_ogg(&mut output)?;

    // Validate the output
    validate(&output)?;

    Ok(output)
}

fn try_convert_with_auto_detection(
    input_path: &PathBuf,
    inline_codebooks: bool,
    full_setup: bool,
    force_packet_format: ForcePacketFormat,
) -> Result<Vec<u8>, WemError> {
    // Try default codebooks first
    match convert_file(
        input_path,
        CodebookLibrary::default_codebooks()?,
        inline_codebooks,
        full_setup,
        force_packet_format,
    ) {
        Ok(data) => return Ok(data),
        Err(e) => {
            eprintln!("Default codebooks failed: {}", e);
        }
    }

    // Try aoTuV codebooks
    eprintln!("Trying aoTuV codebooks...");
    convert_file(
        input_path,
        CodebookLibrary::aotuv_codebooks()?,
        inline_codebooks,
        full_setup,
        force_packet_format,
    )
}
