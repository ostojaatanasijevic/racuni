use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
//QR računi
pub struct Args {
    #[arg(short, long)]
    // Poredi potrošnju na mesečnoj bazi
    pub mesecno: bool,
    pub ukupno: bool,
}
