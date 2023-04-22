use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
//QR računi
pub struct Args {
    #[arg(short, long)]
    ///Lenght of fft sample size
    pub mesecno: bool,
}
