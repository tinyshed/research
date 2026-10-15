use rusqlite::{Connection, params};
use rust_stemmers::{Algorithm, Stemmer};

#[derive(Clone, Debug)]
pub struct Doc {
    pub id: i64,
    pub body: String,
    pub terms: String,
    pub category: i64,
    pub score: f64,
    pub x: f64,
    pub y: f64,
    pub other: i64,
}
pub const SCHEMA:&str="create table documents(id integer primary key,body text not null,terms text not null,
    category integer,score real,x real,y real,other integer default 0);
    create table generation(id integer primary key,kind text,state text,algorithm text,high integer,frontier integer,
        definition integer not null,active integer not null default 0);
    create table dirty(id integer primary key);
    create table seen(generation integer,id integer,primary key(generation,id)) without rowid;";

pub fn normalize(text: &str, stem: bool) -> String {
    let lower = text.to_lowercase().replace('ё', "е");
    let stemmer = Stemmer::create(Algorithm::Russian);
    lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|x| !x.is_empty())
        .map(|x| {
            if stem {
                stemmer.stem(x).into_owned()
            } else {
                x.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
pub struct Fixture {
    pub distribution: String,
    pub russian: Vec<String>,
}
impl Fixture {
    pub fn new(distribution: &str) -> Self {
        let russian = if distribution == "russian" {
            std::fs::read_to_string(std::env::var("RUSSIAN_CORPUS").expect("fetch corpus first"))
                .unwrap()
                .lines()
                .map(str::to_owned)
                .collect()
        } else {
            vec![]
        };
        Self {
            distribution: distribution.to_owned(),
            russian,
        }
    }
    pub fn doc(&self, id: i64) -> Doc {
        let i = id.unsigned_abs();
        let h = mix(i);
        let body = match self.distribution.as_str() {
            "russian" => format!(
                "{} документ{} заказ{}",
                self.russian[i as usize % self.russian.len()],
                i,
                i % 1000
            ),
            "zipf" => {
                let mut words = vec![];
                for j in 0..24 {
                    let r = mix(h + j);
                    let rank = if r % 10 < 8 { r % 100 } else { r % 100000 };
                    words.push(format!("term{rank}"));
                }
                format!("{} tenant{} unique{i}", words.join(" "), i % 100)
            }
            _ => format!(
                "tenant{} order payment shipment customer{} invoice region{} status active product{} unique{i}",
                i % 100,
                i % 1000,
                i % 20,
                i % 500
            ),
        };
        let (x, y) = if self.distribution == "clustered" {
            (
                37.6 + (h % 10000) as f64 / 1e6,
                55.7 + (mix(h) % 10000) as f64 / 1e6,
            )
        } else {
            (
                (h % 36000000) as f64 / 100000. - 180.,
                (mix(h) % 18000000) as f64 / 100000. - 90.,
            )
        };
        Doc {
            id,
            terms: normalize(&body, self.distribution == "russian"),
            body,
            category: (i % 100) as i64,
            score: (h % 100000) as f64 / 100.,
            x,
            y,
            other: 0,
        }
    }
}
pub fn mix(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58476d1ce4e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
pub fn insert(c: &Connection, d: &Doc) {
    c.prepare_cached("insert into documents(id,body,terms,category,score,x,y,other) values(?1,?2,?3,?4,?5,?6,?7,?8)").unwrap()
        .execute(params![d.id,d.body,d.terms,d.category,d.score,d.x,d.y,d.other]).unwrap();
}
pub fn read(c: &Connection, lo: Option<i64>, hi: i64, limit: usize) -> Vec<Doc> {
    if lo == Some(i64::MAX) {
        return vec![];
    }
    let lower = lo.map(|id| id + 1).unwrap_or(i64::MIN);
    c.prepare_cached("select id,body,terms,category,score,x,y,other from documents where id>=?1 and id<=?2 order by id limit cast(?3 as integer)")
        .unwrap().query_map(params![lower,hi,limit as i64],|r|Ok(Doc{id:r.get(0)?,body:r.get(1)?,terms:r.get(2)?,category:r.get(3)?,score:r.get(4)?,x:r.get(5)?,y:r.get(6)?,other:r.get(7)?}))
        .unwrap().map(Result::unwrap).collect()
}
pub fn seed(c: &Connection, n: usize, shape: &str, order: &str, batch: usize) {
    c.execute_batch(SCHEMA).unwrap();
    let f = Fixture::new(shape);
    let mut ids: Vec<usize> = (1..=n).collect();
    if order == "shuffled" {
        ids.sort_unstable_by_key(|i| mix(*i as u64));
    }
    for chunk in ids.chunks(batch) {
        c.execute_batch("begin immediate").unwrap();
        for id in chunk {
            insert(c, &f.doc(*id as i64));
        }
        c.execute_batch("commit").unwrap();
        if crate::common::wal_bytes(&crate::common::path()) > 64 << 20 {
            crate::common::checkpoint(c);
        }
    }
    crate::common::checkpoint(c);
}
