use crate::{common::*, fixture, native};
use serde_json::json;
use std::time::Instant;

pub fn run(a: &[String]) {
    let kind = &a[0];
    let n: usize = a[1].parse().unwrap();
    let shape = &a[2];
    let c = open(&path());
    if !a.get(3).is_some_and(|s| s == "prepared") {
        fixture::seed(&c, n, shape, "ordered", 1000);
    }
    let idle = memory();
    let cpu0 = cpu();
    let begin = Instant::now();
    let mut times = vec![];
    let mut samples = vec![];
    let mut checksum = 0u64;
    #[cfg(feature = "geo")]
    if kind == "rstar" {
        use rstar::{AABB, RTree, primitives::GeomWithData};
        let points: Vec<_> = c
            .prepare("select id,x,y from documents order by id")
            .unwrap()
            .query_map([], |r| {
                Ok(GeomWithData::new(
                    [r.get::<_, f64>(1)?, r.get::<_, f64>(2)?],
                    r.get::<_, i64>(0)?,
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let load_us = us(begin);
        let tree = RTree::bulk_load(points);
        let build_us = us(begin);
        let resident = memory();
        for q in 0..300 {
            let t = Instant::now();
            let (x0, x1, y0, y1) = native::rect(q, shape);
            let mut ids: Vec<i64> = tree
                .locate_in_envelope(&AABB::from_corners([x0, y0], [x1, y1]))
                .map(|v| v.data)
                .collect();
            ids.sort_unstable();
            let elapsed = us(t);
            times.push(elapsed);
            samples.push(json!({"q":q,"bucket":q%4,"us":elapsed,"matched":ids.len()}));
            checksum = checksum.wrapping_mul(0x100000001b3) ^ hash_ids(&ids);
            if q < 20 {
                assert_eq!(ids, native::query(&c, 0, "scan-geo", q, shape));
            }
        }
        let query_us: u64 = times.iter().sum();
        let t = Instant::now();
        let mut tree = tree;
        let old: Vec<_> = c
            .prepare("select id,x,y from documents order by id limit 1000")
            .unwrap()
            .query_map([], |r| {
                Ok(GeomWithData::new(
                    [r.get::<_, f64>(1)?, r.get::<_, f64>(2)?],
                    r.get::<_, i64>(0)?,
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for d in old {
            assert!(tree.remove(&d).is_some());
            let p = *d.geom();
            tree.insert(GeomWithData::new([p[0] + 1e-7, p[1]], d.data));
        }
        println!(
            "{}",
            json!({"experiment":"alternative","kind":kind,"rows":n,"shape":shape,"load_us":load_us,"build_us":build_us,"queries":distribution(&mut times),"query_samples":samples,"query_per_s":300e6/query_us.max(1) as f64,"query_checksum":checksum,"update_1000_us":us(t),"idle_memory":idle,"resident_memory":resident,"final_memory":memory(),"cpu_s_including_oracles":cpu()-cpu0,"allocations":allocations(),"durability":"rebuild-from-source cache; not transactional storage","status":"pass"})
        );
        return;
    }
    #[cfg(feature = "bitmap")]
    if kind == "roaring" {
        let mut maps: Vec<roaring::RoaringTreemap> =
            (0..100).map(|_| roaring::RoaringTreemap::new()).collect();
        let mut scores = vec![0.; n + 1];
        let mut stmt = c
            .prepare("select id,category,score from documents order by id")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let id: i64 = row.get(0).unwrap();
            let cat: i64 = row.get(1).unwrap();
            assert!(id > 0 && id <= n as i64);
            maps[cat as usize].insert(id as u64);
            scores[id as usize] = row.get(2).unwrap();
        }
        let build_us = us(begin);
        let resident = memory();
        let serialized: u64 = maps.iter().map(|m| m.serialized_size() as u64).sum();
        for q in 0..300 {
            let t = Instant::now();
            let ids: Vec<i64> = maps[q % 100]
                .iter()
                .filter(|id| scores[*id as usize] >= (q % 8 * 100) as f64)
                .map(|id| id as i64)
                .collect();
            let elapsed = us(t);
            times.push(elapsed);
            samples.push(json!({"q":q,"bucket":q%8,"us":elapsed,"matched":ids.len()}));
            checksum = checksum.wrapping_mul(0x100000001b3) ^ hash_ids(&ids);
            if q < 20 {
                assert_eq!(ids, native::query(&c, 0, "scan-secondary", q, shape));
            }
        }
        let query_us: u64 = times.iter().sum();
        let t = Instant::now();
        for i in 1..=1000 {
            maps[i % 100].remove(i as u64);
            maps[(i % 100 + 1) % 100].insert(i as u64);
        }
        println!(
            "{}",
            json!({"experiment":"alternative","kind":kind,"rows":n,"shape":shape,"build_us":build_us,"queries":distribution(&mut times),"query_samples":samples,"query_per_s":300e6/query_us.max(1) as f64,"query_checksum":checksum,"update_1000_us":us(t),"serialized_bitmap_bytes":serialized,"score_array_bytes":scores.len()*8,"idle_memory":idle,"resident_memory":resident,"final_memory":memory(),"cpu_s_including_oracles":cpu()-cpu0,"allocations":allocations(),"durability":"rebuild-from-source cache; not transactional storage","status":"pass"})
        );
        return;
    }
    panic!("unavailable alternative {kind}");
}
