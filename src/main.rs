use std::fs::{File, OpenOptions};
use std::io::Write;
use std::io::{self, BufRead};
use urlencoding::decode;

pub const FILEPATH: &str = "/var/log/nginx/access.log";
pub const PROCESSED_LINKS_PATH: &str = "links.done";

struct Artikl {
    ime: String,
    cena: f32,
    komada: f32,
    ukupna_cena: f32,
}

struct Racun {
    vreme: String,
    lista_artikala: Vec<Artikl>,
}

impl Eq for Racun {}
impl PartialEq for Racun {
    fn eq(&self, other: &Self) -> bool {
        self.vreme == other.vreme
    }
}

impl Ord for Racun {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let halfs = self.vreme.split("--");
        let date = halfs
            .clone()
            .nth(0)
            .unwrap()
            .split(".")
            .map(|a| a.parse::<i32>().unwrap())
            .collect::<Vec<i32>>();
        let time = halfs
            .clone()
            .nth(1)
            .unwrap()
            .split(":")
            .map(|a| a.parse::<i32>().unwrap())
            .collect::<Vec<i32>>();

        let o_halfs = other.vreme.split("--");

        let o_date = o_halfs
            .clone()
            .nth(0)
            .unwrap()
            .split(".")
            .map(|a| a.parse::<i32>().unwrap())
            .collect::<Vec<i32>>();
        let o_time = o_halfs
            .clone()
            .nth(1)
            .unwrap()
            .split(":")
            .map(|a| a.parse::<i32>().unwrap())
            .collect::<Vec<i32>>();

        date[2].cmp(&o_date[2]).then_with(|| {
            date[1].cmp(&o_date[1]).then_with(|| {
                date[0].cmp(&o_date[0]).then_with(|| {
                    time[0].cmp(&o_time[0]).then_with(|| {
                        time[1]
                            .cmp(&o_time[1])
                            .then_with(|| time[2].cmp(&o_time[2]))
                    })
                })
            })
        })
    }
}
impl PartialOrd for Racun {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn main() {
    //pročitaj sve već uradjene linkove, ne valja duplirati posao
    let (links_file, links_file_empty) = match File::open(PROCESSED_LINKS_PATH) {
        Ok(file) => (file, false),
        Err(_) => {
            eprintln!("Fajl u kojem se čuvaju prethodni linkovi je ili pomeren ili ne postoji, otvara se novi fajl");
            (
                File::create(PROCESSED_LINKS_PATH).expect("Neuspelo kreiranje fajla"),
                true,
            )
        }
    };

    let links_lines_iterator = io::BufReader::new(links_file).lines();
    let mut old_links: Vec<String> = Vec::new();
    if !links_file_empty {
        for link in links_lines_iterator {
            old_links.push(link.unwrap());
        }
    }

    //Bubble sortish al dovoljno dobro za sad
    //plan je da se proveri da li je linija iz for-a dole sadržana u fajlu gore
    let file = File::open(FILEPATH).unwrap();
    let lines_iterator = io::BufReader::new(file).lines();

    //append
    //processed links to old links file
    let mut links_file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(PROCESSED_LINKS_PATH)
        .expect("Failed to open");

    //tuple tipa (dekodirani_link, kodirani_link), neophodno za lakšu proveru već obradjenih linija
    let mut list_of_links: Vec<(String, String)> = Vec::new();
    'log_iter: for line in lines_iterator {
        if let Ok(line) = line {
            for stari_link in old_links.clone() {
                if line == stari_link {
                    continue 'log_iter;
                }
            }

            let result = process_access_log_line(line.clone());
            match result {
                Ok(link) => list_of_links.push((link, line)),
                Err(_) => writeln!(links_file, "{}", line).expect("Failed to write???? WTF"),
            }
        }
    }

    let mut suma_sumarom = 0.0;
    let mut handles = Vec::new();

    //PARALELIZOVANO, AL VALJA POPRAVITI
    //RC il slično za real time upise u fajl
    //ili, bolje još, handling za ctrl+c tako da kulturno quituje
    let len = list_of_links.len();
    for (n, link) in list_of_links.into_iter().enumerate() {
        handles.push(std::thread::spawn(move || {
            let mut racun = Racun {
                vreme: String::new(),
                lista_artikala: Vec::new(),
            };
            let mut suma_sumarom = 0.0;
            let client = reqwest::blocking::Client::builder()
                .user_agent("Mozilla/5.0(X11;Linux x86_64;rv10.0)")
                .build()
                .unwrap();

            let response = client.get(&link.0).send().expect("Failed to download");

            let body = response.text();

            let mut html_data = String::new();
            match body {
                Ok(body) => html_data = body,
                Err(e) => eprintln!("{e}"),
            }

            if !html_data.contains("ФИСКАЛНИ РАЧУН") {
                eprintln!("Ne sadrži fiskalni račun, nešto ne valja");
            }

            let artikli_index = html_data
                .find("Артикли")
                .expect("Nije pronašao ključnu reč Artikli");
            let ukupan_iznos_index = html_data
                .find("Укупан износ:")
                .expect("Nije pronašao ključnu reč iznos");

            let tty = &html_data[artikli_index..ukupan_iznos_index];
            let lines = tty.lines().skip(3);

            /*
            println!("\nZA SVRHE PROVERE:");
            for line in lines.clone() {
                println!("{}", &line);
            }
            */

            let datum = html_data
                .find("ПФР време:")
                .expect("Nekim čudom ovaj račun nema datum");
            let datum = html_data[datum..]
                .lines()
                .nth(0)
                .expect("Nekako nema ni jedne linije")
                .split_once(":")
                .expect("Nema dvotačke, baš čudno")
                .1
                .replace(". ", "--");

            let datum = datum.split_whitespace().last().unwrap();

            racun.vreme = datum.to_string();
            println!("{datum}");

            let mut ime_artikla: String = String::new();

            //ova for petlja prolazi kroz sve linije fiskalnog računa
            for line in lines {
                let line_nums = line.replace(".", "").replace(",", ".");
                let brojevi = line_nums.split(" ");
                let mut cene = Vec::new();

                //ova petlja traži tri broja: cena:broj_komada:ukupna_cena
                for broj in brojevi {
                    let vrednost = broj.parse::<f32>();
                    match vrednost {
                        Ok(v) => cene.push(v),
                        Err(_) => (),
                    }
                }

                if cene.len() != 3 {
                    //eprintln!("Ovo nije red cena");
                    ime_artikla.push_str(line);
                    continue;
                }

                //poslednja provera da li je u pitanju red cena
                if (cene[0] * cene[1] - cene[2]).abs() > 0.1 {
                    //eprintln!("Ovo nije red cena");
                    ime_artikla.push_str(line);
                    continue;
                }

                println!(
                    "Kupljeni artikl je {ime_artikla}\nkomada {} po ukupnoj ceni od {}",
                    cene[1], cene[2]
                );

                racun.lista_artikala.push(Artikl {
                    ime: ime_artikla,
                    cena: cene[0],
                    komada: cene[1],
                    ukupna_cena: cene[2],
                });

                suma_sumarom += cene[2];
                ime_artikla = String::new();
            }

            (link.1.clone(), suma_sumarom, n, racun)
        }));
    }

    let mut lista_racuna = Vec::new();
    let mut to_write = vec![String::new(); len];
    for handle in handles {
        let tmp = handle.join().unwrap();
        to_write[tmp.2] = tmp.0;
        suma_sumarom += tmp.1;
        lista_racuna.push(tmp.3)
    }

    for line in to_write {
        writeln!(links_file, "{}", line).unwrap();
    }

    println!("==========================================\nSveukupno u navedenom periodu potrošeno: {}\n==========================================",suma_sumarom);

    lista_racuna.sort();

    let mut out_file = match OpenOptions::new().write(true).append(true).open("out") {
        Ok(file) => file,
        Err(_) => File::create("out").expect("Neuspelo kreiranje fajla"),
    };

    for racun in lista_racuna {
        writeln!(out_file, "{}", racun.vreme).unwrap();

        for artikl in racun.lista_artikala.iter() {
            writeln!(
                out_file,
                "Kupljeni artikl je {}\nkomada {} po ukupnoj ceni od {}, {} po komadu",
                artikl.ime, artikl.komada, artikl.ukupna_cena, artikl.cena
            )
            .unwrap();
        }
    }
    writeln!(out_file,"==========================================\nSveukupno u navedenom periodu potrošeno: {}\n==========================================",suma_sumarom).unwrap();
}

fn process_access_log_line(line: String) -> Result<String, String> {
    if !line.contains("suf.purs.gov.rs") {
        return Err("Ne sadrži suf.purs.gov.rs".to_string());
    }

    let link_raw = line
        .splitn(2, "\"")
        .nth(1)
        .expect("Ne postoji 2gi element: Nešto veoma čudno se desilo");

    let start_index = link_raw.find("https").expect("Nema https: Ovo nije link");
    let stop_index = link_raw
        .find(" HTTP")
        .expect("Nema HTTP na kraju: Ovo je atipičan link");

    let mut link = decode(&link_raw[start_index..stop_index])
        .expect("Url dekodiranje je neuspešno")
        .into_owned();

    let end = link.find("&format");
    match end {
        Some(end_index) => link = link[0..end_index].to_string(),
        None => link = link,
    }

    Ok(link)
}
