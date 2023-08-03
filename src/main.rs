mod http;
use semver::Version;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, path::PathBuf};
use std::{thread, time};

#[derive(serde::Deserialize, serde::Serialize)]
struct CdnFile {
    name: String,
    size: u32,
    hash: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Game<'a> {
    engine: &'a str,
    client: &'a str,
    references: Vec<&'a str>,
}

const MASTER: &str = "https://master.alterware.dev";
const REPO: &str = "mxve/alterware-launcher";

fn get_cache_buster() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(n) => n.as_secs(),
        Err(_) => 1,
    }
}

fn get_file_sha1(path: &PathBuf) -> String {
    let mut sha1 = sha1_smol::Sha1::new();
    sha1.update(&fs::read(path).unwrap());
    sha1.digest().to_string()
}

fn check_for_launcher_update() {
    let current_version: Version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    let github_body = http::get_body_string(
        format!("https://api.github.com/repos/{}/releases/latest", REPO).as_str(),
    );
    let github_json: serde_json::Value = serde_json::from_str(&github_body).unwrap();
    let latest_version = github_json["tag_name"]
        .to_string()
        .replace(['v', '"'].as_ref(), "");
    let latest_version = Version::parse(&latest_version).unwrap();

    if current_version < latest_version {
        println!(
            "A new version of the AlterWare launcher is available: {}",
            latest_version
        );
        println!("Download it at https://github.com/{}/releases/latest", REPO);
        println!("Launching in 10 seconds..");
        thread::sleep(time::Duration::from_secs(10));
    }
}

fn update(game: &Game) {
    let cdn_info: Vec<CdnFile> = serde_json::from_str(&http::get_body_string(
        format!("{}/files.json?{}", MASTER, get_cache_buster()).as_str(),
    ))
    .unwrap();

    for file in cdn_info {
        if !file.name.starts_with(game.engine) {
            continue;
        }

        let file_path = PathBuf::from(&file.name.replace(&format!("{}/", game.engine), ""));
        if file_path.exists() {
            let sha1_local = get_file_sha1(&file_path).to_lowercase();
            let sha1_remote = file.hash.to_lowercase();
            if sha1_local != sha1_remote {
                println!(
                    "Updating {}...\nLocal hash: {}\nRemote hash: {}",
                    file_path.display(),
                    sha1_local,
                    sha1_remote
                );
                http::download_file(
                    &format!("{}/{}?{}", MASTER, file.name, get_cache_buster()),
                    &file_path,
                );
            }
        } else {
            println!("Downloading {}...", file_path.display());
            if let Some(parent) = file_path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent).unwrap();
                }
            }
            http::download_file(
                &format!("{}/{}?{}", MASTER, file.name, get_cache_buster()),
                &file_path,
            );
        }
    }
}

fn launch(file_path: &PathBuf) {
    println!("Launching {}...", file_path.display());
    std::process::Command::new(file_path)
        .spawn()
        .expect("Failed to launch the game")
        .wait()
        .expect("Failed to wait for the game process to finish");
}

fn main() {
    check_for_launcher_update();

    let mut args: Vec<String> = std::env::args().collect();

    let games_json = http::get_body_string(format!("{}/games.json", MASTER).as_str());
    let games: Vec<Game> = serde_json::from_str(&games_json).unwrap();

    let mut update_only = false;
    if args.contains(&String::from("update")) {
        update_only = true;
        args.iter()
            .position(|r| r == "update")
            .map(|e| args.remove(e));
    }

    let mut game: String = String::new();
    if args.len() > 1 {
        game = String::from(&args[1]);
    } else {
        'main: for g in games.iter() {
            for r in g.references.iter() {
                if std::path::Path::new(r).exists() {
                    game = String::from(g.client);
                    break 'main;
                }
            }
        }
    }

    for g in games.iter() {
        if g.client == game {
            update(g);
            if update_only {
                return;
            }
            launch(&PathBuf::from(format!("{}.exe", g.client)));
            return;
        }
    }

    println!("Game not found!");
    println!("Place the launcher in the game folder, if that doesn't work specify the client on the command line (ex. alterware-launcher.exe iw4-sp)");
    println!("Press enter to exit...");
    std::io::stdin().read_line(&mut String::new()).unwrap();
}
