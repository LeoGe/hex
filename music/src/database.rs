use error::{Error, Result};

use rusqlite;
use rusqlite::Statement;

use music_search::SearchQuery;
use audio_file::AudioFile;
use acousticid;

use std::{env, str};
use std::fs::File;
use std::io::Write;
use std::process::Command;

use uuid::Uuid;

#[derive(Serialize, Clone, Debug)]
pub struct Track {
    title: Option<String>,
    album: Option<String>,
    interpret: Option<String>,
    conductor: Option<String>,
    composer: Option<String>,
    fingerprint: String,
    pub key: String,
    pub duration: f64,
    favs_count: u32,
    channels: u32
}

#[derive(Serialize, Clone, Debug)]
pub struct Playlist {
    pub key: String,
    pub title: String,
    desc: Option<String>,
    count: u32
}

impl Track {
    pub fn empty(key: &str, fingerprint: &str, duration: f64) -> Track {
        Track {
            key: key.into(),
            fingerprint: fingerprint.into(),
            duration: duration,
            title: None,
            album: None,
            interpret: None,
            conductor: None,
            composer: None,
            favs_count: 0,
            channels: 2
        }
    }

    pub fn new(key: &str, fingerprint: &str, duration: f64, title: Option<String>, album: Option<String>, interpret: Option<String>, conductor: Option<String>, composer: Option<String>, favs_count: u32, channels: u32) -> Track {
        Track {
            key: key.into(),
            fingerprint: fingerprint.into(),
            duration: duration,
            title: title,
            album: album,
            interpret: interpret,
            conductor: conductor,
            composer: composer,
            favs_count: favs_count,
            channels: channels
        }
    }

    pub fn suggestion(&self) -> Result<String> {
        acousticid::get_metadata(&self.fingerprint, self.duration as u32)
    }
}

pub struct Connection {
    socket: rusqlite::Connection
}

impl Connection {
    pub fn new() -> Connection {
        let mut dir = env::home_dir().expect("Could not find the home directory!");
        dir.push(".music.db");

        Connection { socket: rusqlite::Connection::open(dir.to_str().unwrap()).unwrap() }
    }

    pub fn search_prep(&self, query: SearchQuery) -> Statement {
        if query.is_empty() {
            self.socket.prepare("SELECT Title, Album, Interpret, Conductor, Composer, Key, Duration, FavsCount, Channels FROM music").unwrap()
        } else {
            let query = query.to_sql_query();

            println!("Query: {}", query);
            self.socket.prepare(&format!("SELECT Title, Album, Interpret, Fingerprint, Conductor, Composer, Key, Duration, FavsCount, Channels FROM music WHERE {};", query)).unwrap()
        }
    }

    pub fn search<'a>(&self, stmt: &'a mut Statement) -> impl Iterator<Item = Track> + 'a {
        stmt.query_map(&[], |row| {
            Track {
                title: row.get(0),
                album: row.get(1),
                interpret: row.get(2),
                fingerprint: row.get(3),
                conductor: row.get(4),
                composer: row.get(5),
                key: row.get(6),
                duration: row.get(7),
                favs_count: row.get(8),
                channels: row.get(9)
            }
        }).unwrap().filter_map(|x| x.ok()).map(|x| x.clone())
    }

    pub fn get_playlists(&self) -> Vec<Playlist> {
        let mut tmp = self.socket.prepare("SELECT Key, Title, Desc, Count FROM Playlists").unwrap();
        let res = tmp.query_map(&[], |row| {
                Playlist {
                    key: row.get(0),
                    title: row.get(1),
                    desc: row.get(2),
                    count: row.get(3)
                }
            }).unwrap().filter_map(|x| x.ok()).collect();

        res
    }

    pub fn get_playlist(&self, key: &str) -> (Playlist, Vec<Track>) {
        let mut stmt = self.socket.prepare("SELECT Key, Title, Desc, Count, tracks FROM Playlists WHERE key=?;").unwrap();
        let mut rows = stmt.query(&[&key]).unwrap();
        let row = rows.next().unwrap().unwrap();

        let playlist = Playlist {
            key: row.get(0),
            title: row.get(1),
            desc: row.get(2),
            count: row.get(3)
        };

        let keys: Option<String> = row.get(4);

        println!("Got keys: {:?}", keys);

        if let Some(keys) = keys {
            let query = format!("SELECT Title, Album, Interpret, Fingerprint, Conductor, Composer, Key, Duration, FavsCount, Channels FROM music WHERE key in ({});", keys.split(",").map(|row| { format!("'{}'", row) }).collect::<Vec<String>>().join(","));
            println!("{}", query);

            let mut stmt = self.socket.prepare(&query).unwrap();

            let res = self.search(&mut stmt).collect();

            (playlist, res)
        } else {
            (playlist, Vec::new())
        }
    }

    pub fn get_playlists_of_track(&self, key: &str) -> Vec<Playlist> {
        let mut stmt = self.socket.prepare(&format!("SELECT Key, Title, Desc, Count FROM Playlists WHERE tracks like '%{}%'", key)).unwrap();
        let res = stmt.query_map(&[], |row| {
            Playlist {
                key: row.get(0),
                title: row.get(1),
                desc: row.get(2),
                count: row.get(3)
            }
        }).unwrap().filter_map(|x| x.ok()).collect();

        res
    }


    pub fn add_playlist(&self, title: &str) -> Playlist {
        let key = Uuid::new_v4().simple().to_string();

        self.socket.execute("INSERT INTO playlists (key, title, count) VALUES (?1, ?2, ?3)", &[&key, &title, &0]).unwrap();

        Playlist {
            key: key,
            title: title.into(),
            desc: None,
            count: 0
        }
    }

    pub fn insert_track(&self, track: Track) {
        self.socket.execute("INSERT INTO music (Title, Album, Interpret, Conductor, Composer, Key, Fingerprint, Duration, FavsCount, Channels) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)", &[&track.title, &track.album, &track.interpret, &track.conductor, &track.composer, &track.key, &track.fingerprint, &track.duration, &track.favs_count, &track.channels]).unwrap();
    }

    pub fn get_track(&self, key: &str) -> Result<Track> {
        let mut stmt = self.socket.prepare(&format!("SELECT Title, Album, Interpret, Fingerprint, Conductor, Composer, Key, Duration, FavsCount, Channels FROM music WHERE Key = '{}'", key)).map_err(|_| Error::Internal)?;
        
        let res = self.search(&mut stmt).next().ok_or(Error::Internal);

        res
    }
    pub fn update_track(&self, key: &str, title: Option<String>, album: Option<String>, interpret: Option<String>, conductor: Option<String>, composer: Option<String>) -> Result<String> {
        if let Some(title) = title {
            self.socket.execute("UPDATE music SET Title = ? WHERE Key = ?", &[&title, &key]).map_err(|_| Error::Internal)?;
        }
        if let Some(album) = album {
            self.socket.execute("UPDATE music SET Album = ? WHERE Key = ?", &[&album, &key]).map_err(|_| Error::Internal)?;
        }
        if let Some(interpret) = interpret {
            self.socket.execute("UPDATE music SET Interpret = ? WHERE Key = ?", &[&interpret, &key]).map_err(|_| Error::Internal)?;
        }
        if let Some(conductor) = conductor {
            self.socket.execute("UPDATE music SET Conductor = ? WHERE Key = ?", &[&conductor, &key]).map_err(|_| Error::Internal)?;
        }
        if let Some(composer) = composer {
            self.socket.execute("UPDATE music SET Composer = ? WHERE Key = ?", &[&composer, &key]).map_err(|_| Error::Internal)?;
        }

        return Ok(key.into());
    
    }
}
