//! A LightGBM model read from its own text file and scored as LightGBM scores
//! a numerical tree: a missing value as its missing type says, else the value
//! against the threshold. Features pass through `f32` first, as Python's
//! matrix of them does.

use std::collections::HashMap;

pub struct Tree {
    feature: Vec<usize>,
    threshold: Vec<f64>,
    decision: Vec<u8>,
    left: Vec<i64>,
    right: Vec<i64>,
    leaf: Vec<f64>,
}

pub struct Model {
    pub names: Vec<String>,
    trees: Vec<Tree>,
}

fn list<T: std::str::FromStr>(v: Option<&&str>) -> Vec<T> {
    v.map(|s| s.split_whitespace().filter_map(|x| x.parse().ok()).collect()).unwrap_or_default()
}

impl Model {
    pub fn parse(text: &str) -> Result<Model, String> {
        let mut names = Vec::new();
        let mut trees = Vec::new();
        let mut block: HashMap<&str, &str> = HashMap::new();
        let mut in_tree = false;
        let flush = |block: &mut HashMap<&str, &str>, trees: &mut Vec<Tree>| {
            if block.is_empty() {
                return;
            }
            trees.push(Tree {
                feature: list(block.get("split_feature")),
                threshold: list(block.get("threshold")),
                decision: list(block.get("decision_type")),
                left: list(block.get("left_child")),
                right: list(block.get("right_child")),
                leaf: list(block.get("leaf_value")),
            });
            block.clear();
        };
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("feature_names=") {
                names = rest.split_whitespace().map(str::to_string).collect();
            } else if line.starts_with("Tree=") {
                flush(&mut block, &mut trees);
                in_tree = true;
            } else if line.starts_with("end of trees") {
                flush(&mut block, &mut trees);
                in_tree = false;
            } else if in_tree {
                if let Some((k, v)) = line.split_once('=') {
                    block.insert(k, v);
                }
            }
        }
        flush(&mut block, &mut trees);
        if names.is_empty() || trees.is_empty() {
            return Err("not a LightGBM model".into());
        }
        Ok(Model { names, trees })
    }

    pub fn load(path: &std::path::Path) -> Result<Model, String> {
        Model::parse(&std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?)
    }

    /// A question's score from its features by name; a name it lacks is missing.
    pub fn score(&self, f: &HashMap<&'static str, f64>) -> f64 {
        let x: Vec<f64> = self.names.iter().map(|n| f.get(n.as_str()).map(|&v| (v as f32) as f64).unwrap_or(f64::NAN)).collect();
        self.trees.iter().map(|t| t.score(&x)).sum()
    }
}

impl Tree {
    fn score(&self, x: &[f64]) -> f64 {
        if self.feature.is_empty() {
            return self.leaf.first().copied().unwrap_or(0.0);
        }
        let mut node: i64 = 0;
        loop {
            let i = node as usize;
            let mut v = x[self.feature[i]];
            let missing = (self.decision[i] >> 2) & 3;
            let default_left = self.decision[i] & 2 != 0;
            if v.is_nan() && missing != 2 {
                v = 0.0;
            }
            node = if (missing == 1 && v.abs() <= 1e-35) || (missing == 2 && v.is_nan()) {
                if default_left { self.left[i] } else { self.right[i] }
            } else if v <= self.threshold[i] {
                self.left[i]
            } else {
                self.right[i]
            };
            if node < 0 {
                return self.leaf[(-node - 1) as usize];
            }
        }
    }
}
