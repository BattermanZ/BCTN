use std::collections::HashMap;

use anyhow::Result;
use rusqlite::Connection;

pub(super) type YearSeries = HashMap<(i64, i32), i64>;
pub(super) type SexTotals = HashMap<(i64, String), i64>;
pub(super) type CountryTotalsAll = HashMap<(i64, String), i64>;

pub(super) fn string_score_from_distance(dist: usize) -> f64 {
    match dist {
        0 => 1.0,
        1 => 0.98,
        2 => 0.93,
        3 => 0.88,
        _ => 0.0,
    }
}

pub(super) fn trend_correlation_signals(
    series: &YearSeries,
    a: i64,
    b: i64,
    min_years_nonzero: usize,
) -> (f64, Option<f64>, usize, f64, Option<f64>) {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut years = 0usize;
    for year in 2005..=2024 {
        let av = series.get(&(a, year)).copied().unwrap_or(0);
        let bv = series.get(&(b, year)).copied().unwrap_or(0);
        if av > 0 && bv > 0 {
            xs.push(av as f64);
            ys.push(bv as f64);
            years += 1;
        }
    }
    if years < min_years_nonzero {
        return (0.0, None, years, 0.0, None);
    }

    let corr = pearson_corr(&xs, &ys).unwrap_or(0.0).clamp(-1.0, 1.0);
    let corr_score = corr.max(0.0);
    let sum_x: f64 = xs.iter().sum();
    let sum_y: f64 = ys.iter().sum();
    let ratio = if sum_x > 0.0 && sum_y > 0.0 {
        Some(sum_x / sum_y)
    } else {
        None
    };
    let magnitude_score = ratio
        .map(|r| (1.0 - (r.ln().abs() / 2.0_f64.ln())).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    (corr_score, Some(corr), years, magnitude_score, ratio)
}

pub(super) fn pearson_corr(xs: &[f64], ys: &[f64]) -> Option<f64> {
    if xs.len() != ys.len() || xs.is_empty() {
        return None;
    }
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den_x = 0.0;
    let mut den_y = 0.0;
    for i in 0..xs.len() {
        let dx = xs[i] - mean_x;
        let dy = ys[i] - mean_y;
        num += dx * dy;
        den_x += dx * dx;
        den_y += dy * dy;
    }
    let den = (den_x * den_y).sqrt();
    if den == 0.0 { None } else { Some(num / den) }
}

pub(super) fn sex_agreement_score(sex: &SexTotals, a: i64, b: i64) -> f64 {
    let am = sex.get(&(a, "M".to_owned())).copied().unwrap_or(0);
    let af = sex.get(&(a, "F".to_owned())).copied().unwrap_or(0);
    let bm = sex.get(&(b, "M".to_owned())).copied().unwrap_or(0);
    let bf = sex.get(&(b, "F".to_owned())).copied().unwrap_or(0);
    let at = (am + af) as f64;
    let bt = (bm + bf) as f64;
    if at == 0.0 || bt == 0.0 {
        return 0.5;
    }
    let pa = (am as f64) / at;
    let pb = (bm as f64) / bt;
    let diff = (pa - pb).abs();
    (1.0 - (diff / 0.25)).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CollisionInfo {
    pub(super) penalty: f64,
    pub(super) a_total: i64,
    pub(super) b_total: i64,
    pub(super) a_countries_ge_100: i32,
    pub(super) b_countries_ge_100: i32,
}

pub(super) fn collision_info(
    totals: &CountryTotalsAll,
    a: i64,
    b: i64,
    dist: usize,
) -> CollisionInfo {
    let mut a_countries_ge_100 = 0i32;
    let mut b_countries_ge_100 = 0i32;
    let mut a_total = 0i64;
    let mut b_total = 0i64;
    for country in ["US", "FR", "CL"] {
        let av = totals.get(&(a, country.to_owned())).copied().unwrap_or(0);
        let bv = totals.get(&(b, country.to_owned())).copied().unwrap_or(0);
        a_total += av;
        b_total += bv;
        if av >= 100 {
            a_countries_ge_100 += 1;
        }
        if bv >= 100 {
            b_countries_ge_100 += 1;
        }
    }
    let mut penalty = 0.0;
    if (a_countries_ge_100 - b_countries_ge_100).abs() >= 2 {
        penalty += 0.05;
    }
    if dist == 1 && a_total >= 50_000 && b_total >= 50_000 {
        penalty += 0.10;
    }
    CollisionInfo {
        penalty,
        a_total,
        b_total,
        a_countries_ge_100,
        b_countries_ge_100,
    }
}

#[derive(Debug, Default, Clone)]
pub(super) struct ConceptQids {
    primary_qid: Option<String>,
    primary_conflict: bool,
    all_qids: Vec<String>,
}

pub(super) type ConceptQidsByConcept = HashMap<i64, ConceptQids>;

pub(super) fn load_country_concept_qids(conn: &Connection) -> Result<ConceptQidsByConcept> {
    let mut stmt = conn.prepare(
        "SELECT tc.id, w.qid, w.matched_label_lang
         FROM tmp_concept tc
         JOIN wikidata_concept_match w ON w.concept_id = tc.id",
    )?;
    let mut rows = stmt.query([])?;
    let mut out: ConceptQidsByConcept = HashMap::new();
    while let Some(row) = rows.next()? {
        let concept_id: i64 = row.get(0)?;
        let qid: String = row.get(1)?;
        let matched_label_lang: Option<String> = row.get(2)?;

        let entry = out.entry(concept_id).or_default();
        entry.all_qids.push(qid.clone());
        if matched_label_lang.is_none() {
            if entry.primary_conflict {
                continue;
            }
            match &entry.primary_qid {
                None => entry.primary_qid = Some(qid),
                Some(existing) => {
                    if existing != &qid {
                        entry.primary_qid = None;
                        entry.primary_conflict = true;
                    }
                }
            }
        }
    }
    Ok(out)
}

pub(super) fn concepts_share_any_qid_from_map(qids: &ConceptQidsByConcept, a: i64, b: i64) -> bool {
    let Some(aq) = qids.get(&a) else { return false };
    let Some(bq) = qids.get(&b) else { return false };
    if aq.all_qids.len() <= bq.all_qids.len() {
        for q in &aq.all_qids {
            if bq.all_qids.iter().any(|x| x == q) {
                return true;
            }
        }
    } else {
        for q in &bq.all_qids {
            if aq.all_qids.iter().any(|x| x == q) {
                return true;
            }
        }
    }
    false
}

pub(super) fn concepts_share_primary_qid_from_map(
    qids: &ConceptQidsByConcept,
    a: i64,
    b: i64,
) -> bool {
    let Some(aq) = qids.get(&a) else { return false };
    let Some(bq) = qids.get(&b) else { return false };
    match (aq.primary_qid.as_deref(), bq.primary_qid.as_deref()) {
        (Some(qa), Some(qb)) => qa == qb,
        _ => false,
    }
}

pub(super) fn levenshtein_bounded(a: &str, b: &str, max: usize) -> Option<usize> {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let len_a = a.len();
    let len_b = b.len();
    if len_a.abs_diff(len_b) > max {
        return None;
    }
    if len_a == 0 {
        return (len_b <= max).then_some(len_b);
    }
    if len_b == 0 {
        return (len_a <= max).then_some(len_a);
    }

    let mut prev: Vec<usize> = (0..=len_b).collect();
    let mut curr: Vec<usize> = vec![0; len_b + 1];

    for i in 1..=len_a {
        curr[0] = i;
        let mut row_min = curr[0];
        for j in 1..=len_b {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            let del = prev[j] + 1;
            let ins = curr[j - 1] + 1;
            let sub = prev[j - 1] + cost;
            let v = del.min(ins).min(sub);
            curr[j] = v;
            row_min = row_min.min(v);
        }
        if row_min > max {
            return None;
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    let dist = prev[len_b];
    (dist <= max).then_some(dist)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_bounded_basic_cases() {
        assert_eq!(levenshtein_bounded("AYMERIC", "AYMERIC", 2), Some(0));
        assert_eq!(levenshtein_bounded("kitten", "sitting", 2), None);
        assert_eq!(levenshtein_bounded("kitten", "sitting", 3), Some(3));
        assert_eq!(levenshtein_bounded("", "abc", 2), None);
        assert_eq!(levenshtein_bounded("", "abc", 3), Some(3));
    }

    #[test]
    fn pearson_corr_edge_cases() {
        let corr = pearson_corr(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]).unwrap();
        assert!((corr - 1.0).abs() < 1e-12);

        let corr = pearson_corr(&[1.0, 2.0, 3.0], &[3.0, 2.0, 1.0]).unwrap();
        assert!((corr + 1.0).abs() < 1e-12);

        assert_eq!(pearson_corr(&[1.0, 1.0, 1.0], &[2.0, 3.0, 4.0]), None);
        assert_eq!(pearson_corr(&[], &[]), None);
        assert_eq!(pearson_corr(&[1.0], &[1.0, 2.0]), None);
    }
}
