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
    let t = Instant::now();
    let docs = fixture::read(&c, None, i64::MAX, n + 1);
    let load_us = us(t);
    let begin = Instant::now();
    let mut times = vec![];
    let mut checksum = 0;
    let mut matched = 0;
    #[cfg(feature = "geo")]
    if kind == "rstar" {
        use rstar::{AABB, RTree, primitives::GeomWithData};
        let tree = RTree::bulk_load(
            docs.iter()
                .map(|d| GeomWithData::new([d.x, d.y], d.id))
                .collect(),
        );
        let build_us = us(begin);
        let resident = memory();
        let query_start = Instant::now();
        for q in 0..300 {
            let t = Instant::now();
            let (x0, x1, y0, y1) = native::rect(q, shape);
            let mut ids: Vec<i64> = tree
                .locate_in_envelope(&AABB::from_corners([x0, y0], [x1, y1]))
                .map(|v| v.data)
                .collect();
            ids.sort_unstable();
            times.push(us(t));
            checksum ^= hash_ids(&ids);
            matched += ids.len();
            if q < 20 {
                let mut exact: Vec<i64> = docs
                    .iter()
                    .filter(|d| d.x >= x0 && d.x <= x1 && d.y >= y0 && d.y <= y1)
                    .map(|d| d.id)
                    .collect();
                exact.sort_unstable();
                assert_eq!(ids, exact);
            }
        }
        let query_s = query_start.elapsed().as_secs_f64();
        let t = Instant::now();
        let mut tree = tree;
        for d in docs.iter().take(1000) {
            assert!(tree.remove(&GeomWithData::new([d.x, d.y], d.id)).is_some());
            tree.insert(GeomWithData::new([d.x + 1e-7, d.y], d.id));
        }
        println!(
            "{}",
            json!({"experiment":"alternative","kind":kind,"rows":n,"shape":shape,"load_us":load_us,"build_us":build_us,"queries":distribution(&mut times),"query_per_s_including_20_oracles":300./query_s,"query_checksum":checksum,"matched_total":matched,"update_1000_us":us(t),"idle_memory":idle,"resident_memory":resident,"final_memory":memory(),"cpu_s":cpu()-cpu0,"allocations":allocations(),"durability":"rebuild-from-source cache; not transactional storage","status":"pass"})
        );
        return;
    }
    #[cfg(feature = "bitmap")]
    if kind == "roaring" {
        let mut maps: Vec<roaring::RoaringTreemap> =
            (0..100).map(|_| roaring::RoaringTreemap::new()).collect();
        let mut scores = vec![0.; n + 1];
        for d in &docs {
            maps[d.category as usize].insert(d.id as u64);
            scores[d.id as usize] = d.score;
        }
        let build_us = us(begin);
        let resident = memory();
        let serialized: u64 = maps.iter().map(|m| m.serialized_size() as u64).sum();
        let qstart = Instant::now();
        for q in 0..300 {
            let t = Instant::now();
            let ids: Vec<i64> = maps[q % 100]
                .iter()
                .filter(|id| scores[*id as usize] >= (q % 8 * 100) as f64)
                .map(|id| id as i64)
                .collect();
            times.push(us(t));
            checksum ^= hash_ids(&ids);
            matched += ids.len();
            if q < 20 {
                let exact: Vec<i64> = docs
                    .iter()
                    .filter(|d| d.category == q as i64 % 100 && d.score >= (q % 8 * 100) as f64)
                    .map(|d| d.id)
                    .collect();
                assert_eq!(ids, exact);
            }
        }
        let query_s = qstart.elapsed().as_secs_f64();
        let t = Instant::now();
        for d in docs.iter().take(1000) {
            maps[d.category as usize].remove(d.id as u64);
            maps[(d.category as usize + 1) % 100].insert(d.id as u64);
        }
        println!(
            "{}",
            json!({"experiment":"alternative","kind":kind,"rows":n,"shape":shape,"load_us":load_us,"build_us":build_us,"queries":distribution(&mut times),"query_per_s_including_20_oracles":300./query_s,"query_checksum":checksum,"matched_total":matched,"update_1000_us":us(t),"serialized_bitmap_bytes":serialized,"score_array_bytes":scores.len()*8,"idle_memory":idle,"resident_memory":resident,"final_memory":memory(),"cpu_s":cpu()-cpu0,"allocations":allocations(),"durability":"rebuild-from-source cache; not transactional storage","status":"pass"})
        );
        return;
    }
    panic!("unavailable alternative {kind}");
}
