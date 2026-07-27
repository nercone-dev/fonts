use kurbo::Vec2;

extern "C" {
    pub fn hypot(x: f64, y: f64) -> f64;
}

pub fn abs(point: Vec2) -> f64 {
    unsafe { hypot(point.x, point.y) }
}

pub fn divide(point: Vec2, divisor: f64) -> Vec2 {
    Vec2::new(point.x / divisor, point.y / divisor)
}

pub fn cubic_farthest_fit_inside(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, tolerance: f64) -> bool {
    if abs(p2) <= tolerance && abs(p1) <= tolerance {
        return true;
    }

    let mid = (p0 + 3.0 * (p1 + p2) + p3) * 0.125;
    if abs(mid) > tolerance {
        return false;
    }
    let deriv3 = (p3 + p2 - p1 - p0) * 0.125;
    cubic_farthest_fit_inside(p0, (p0 + p1) * 0.5, mid - deriv3, mid, tolerance)
        && cubic_farthest_fit_inside(mid, mid + deriv3, (p2 + p3) * 0.5, p3, tolerance)
}

pub fn elevate_quadratic(p0: Vec2, p1: Vec2, p2: Vec2) -> [Vec2; 4] {
    let p1_2_3 = p1 * (2.0 / 3.0);
    [p0, p0 * (1.0 / 3.0) + p1_2_3, p2 * (1.0 / 3.0) + p1_2_3, p2]
}

pub fn merge_curves(curves: &[[Vec2; 4]], start: usize, n: usize) -> Option<([Vec2; 4], Vec<f64>)> {
    let mut prod_ratio = 1.0;
    let mut sum_ratio = 1.0;
    let mut ts = vec![1.0];
    for k in 1..n {
        let ck = &curves[start + k];
        let c_before = &curves[start + k - 1];

        let denominator = abs(c_before[3] - c_before[2]);
        if denominator == 0.0 {
            return None;
        }
        let ratio = abs(ck[1] - ck[0]) / denominator;

        prod_ratio *= ratio;
        sum_ratio += prod_ratio;
        ts.push(sum_ratio);
    }

    ts.pop();
    let ts: Vec<f64> = ts.iter().map(|t| t / sum_ratio).collect();

    let p0 = curves[start][0];
    let p1 = curves[start][1];
    let p2 = curves[start + n - 1][2];
    let p3 = curves[start + n - 1][3];

    let first = if ts.is_empty() { 1.0 } else { ts[0] };
    let last = if ts.is_empty() { 1.0 } else { 1.0 - ts[ts.len() - 1] };
    if first == 0.0 || last == 0.0 {
        return None;
    }
    let p1 = p0 + divide(p1 - p0, first);
    let p2 = p3 + divide(p2 - p3, last);

    Some(([p0, p1, p2, p3], ts))
}

pub fn add_implicit_on_curves(p: &[Vec2]) -> Vec<Vec2> {
    let mut q = p.to_vec();
    let mut count = 0;
    let num_offcurves = p.len() - 2;
    for i in 1..num_offcurves {
        let off1 = p[i];
        let off2 = p[i + 1];
        let on = off1 + (off2 - off1) * 0.5;
        q.insert(i + 1 + count, on);
        count += 1;
    }
    q
}

pub fn split_cubic_at_t(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, ts: &[f64]) -> Vec<[Vec2; 4]> {
    let c = (p1 - p0) * 3.0;
    let b = (p2 - p1) * 3.0 - c;
    let a = p3 - p0 - c - b;
    let d = p0;

    let mut boundaries = Vec::with_capacity(ts.len() + 2);
    boundaries.push(0.0);
    boundaries.extend_from_slice(ts);
    boundaries.push(1.0);

    let mut segments = Vec::with_capacity(boundaries.len() - 1);
    for i in 0..boundaries.len() - 1 {
        let t1 = boundaries[i];
        let t2 = boundaries[i + 1];
        let delta = t2 - t1;

        let delta_2 = delta * delta;
        let delta_3 = delta * delta_2;
        let t1_2 = t1 * t1;
        let t1_3 = t1 * t1_2;

        let a1 = a * delta_3;
        let b1 = (a * 3.0 * t1 + b) * delta_2;
        let c1 = (b * 2.0 * t1 + c + a * 3.0 * t1_2) * delta;
        let d1 = a * t1_3 + b * t1_2 + c * t1 + d;

        let q2 = c1 * (1.0 / 3.0) + d1;
        let q3 = (b1 + c1) * (1.0 / 3.0) + q2;
        let q4 = a1 + b1 + c1 + d1;
        segments.push([d1, q2, q3, q4]);
    }
    segments
}

#[derive(Clone, Copy)]
pub struct Solution {
    pub num_points: usize,
    pub error: f64,
    pub start_index: usize,
    pub is_cubic: bool,
}

impl Solution {
    pub fn better(&self, other: &Solution) -> bool {
        if self.num_points != other.num_points {
            return self.num_points < other.num_points;
        }
        if self.error != other.error {
            return self.error < other.error;
        }
        if self.start_index != other.start_index {
            return self.start_index < other.start_index;
        }
        !self.is_cubic && other.is_cubic
    }
}

pub fn spline_to_curves(q: &[Vec2], costs: &[usize], tolerance: f64, all_cubic: bool) -> Vec<Vec<Vec2>> {
    assert!(q.len() >= 3, "quadratic spline requires at least 3 points");

    let elevated_quadratics: Vec<[Vec2; 4]> =
        (0..q.len() - 2).step_by(2).map(|i| elevate_quadratic(q[i], q[i + 1], q[i + 2])).collect();

    let mut forced = vec![false; elevated_quadratics.len()];
    for i in 1..elevated_quadratics.len() {
        let p0 = elevated_quadratics[i - 1][2];
        let p1 = elevated_quadratics[i][0];
        let p2 = elevated_quadratics[i][1];
        if abs(p1 - p0) + abs(p2 - p1) > tolerance + abs(p2 - p0) {
            forced[i] = true;
        }
    }

    let mut sols = vec![Solution { num_points: 0, error: 0.0, start_index: 0, is_cubic: false }];
    let impossible = Solution { num_points: elevated_quadratics.len() * 3 + 1, error: 0.0, start_index: 1, is_cubic: false };
    let mut start = 0;
    for i in 1..=elevated_quadratics.len() {
        let mut best_sol = impossible;
        for j in start..i {
            let j_sol_count = sols[j].num_points;
            let j_sol_error = sols[j].error;

            if !all_cubic {
                let this_count = costs[2 * i - 1] - costs[2 * j] + 1;
                let i_sol = Solution { num_points: j_sol_count + this_count, error: j_sol_error, start_index: i - j, is_cubic: false };
                if i_sol.better(&best_sol) {
                    best_sol = i_sol;
                }

                if this_count <= 3 {
                    continue;
                }
            }

            let Some((curve, ts)) = merge_curves(&elevated_quadratics, j, i - j) else {
                continue;
            };

            let reconstructed = split_cubic_at_t(curve[0], curve[1], curve[2], curve[3], &ts);

            let mut error = 0.0f64;
            for (k, reconst) in reconstructed.iter().enumerate() {
                let orig = &elevated_quadratics[j + k];
                error = error.max(abs(reconst[3] - orig[3]));
                if error > tolerance {
                    break;
                }
            }
            if error > tolerance {
                continue;
            }

            for (k, reconst) in reconstructed.iter().enumerate() {
                let orig = &elevated_quadratics[j + k];
                if !cubic_farthest_fit_inside(
                    reconst[0] - orig[0],
                    reconst[1] - orig[1],
                    reconst[2] - orig[2],
                    reconst[3] - orig[3],
                    tolerance,
                ) {
                    error = tolerance + 1.0;
                    break;
                }
            }
            if error > tolerance {
                continue;
            }

            let i_sol = Solution { num_points: j_sol_count + 3, error: j_sol_error.max(error), start_index: i - j, is_cubic: true };
            if i_sol.better(&best_sol) {
                best_sol = i_sol;
            }

            if j_sol_count + 3 == 3 {
                break;
            }
        }
        sols.push(best_sol);
        if i < forced.len() && forced[i] {
            start = i;
        }
    }

    let mut splits = Vec::new();
    let mut cubic = Vec::new();
    let mut i = sols.len() - 1;
    while i > 0 {
        splits.push(i);
        cubic.push(sols[i].is_cubic);
        i -= sols[i].start_index;
    }

    let mut curves = Vec::new();
    let mut j = 0;
    for (i, is_cubic) in splits.iter().zip(cubic.iter()).rev() {
        if *is_cubic {
            let (curve, _) = merge_curves(&elevated_quadratics, j, i - j).expect("chosen solutions are mergeable");
            curves.push(curve.to_vec());
        } else {
            for k in j..*i {
                curves.push(q[k * 2..k * 2 + 3].to_vec());
            }
        }
        j = *i;
    }

    curves
}

pub fn quadratics_to_curves(quads: &[Vec<(f64, f64)>], max_error: f64, all_cubic: bool) -> Vec<Vec<(f64, f64)>> {
    if quads.is_empty() {
        return Vec::new();
    }
    assert!(max_error > 0.0, "max_error must be greater than zero");
    for spline in quads {
        assert!(spline.len() >= 3, "quadratic splines must contain at least 3 points");
    }

    let splines: Vec<Vec<Vec2>> =
        quads.iter().map(|spline| spline.iter().map(|(x, y)| Vec2::new(*x, *y)).collect()).collect();

    let mut q = vec![splines[0][0]];
    let mut costs = vec![1];
    let mut cost = 1;
    for p in &splines {
        assert!(q[q.len() - 1] == p[0], "quadratic splines must connect end-to-start");
        for _ in 0..p.len() - 2 {
            cost += 1;
            costs.push(cost);
            costs.push(cost);
        }
        let qq = add_implicit_on_curves(p);
        costs.pop();
        q.extend_from_slice(&qq[1..]);
        cost += 1;
        costs.push(cost);
    }

    let curves = spline_to_curves(&q, &costs, max_error, all_cubic);
    curves.into_iter().map(|curve| curve.into_iter().map(|point| (point.x, point.y)).collect()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_quadratic_elevates_exactly() {
        let curves = quadratics_to_curves(&[vec![(0.0, 0.0), (50.0, 100.0), (100.0, 0.0)]], 0.1, true);
        assert_eq!(
            curves,
            vec![vec![
                (0.0, 0.0),
                (33.33333333333333, 66.66666666666666),
                (66.66666666666666, 66.66666666666666),
                (100.0, 0.0),
            ]]
        );
    }

    #[test]
    fn merges_match_fonttools_bit_for_bit() {
        let spline = vec![(628.0, -20.0), (475.0, -20.0), (236.0, 93.0), (97.0, 291.0), (94.0, 418.0)];
        let curves = quadratics_to_curves(&[spline], 2048.0 / 2000.0, true);
        assert_eq!(
            curves,
            vec![
                vec![(628.0, -20.0), (526.0, -20.0), (435.16666666666663, -1.1666666666666643), (355.5, 36.5)],
                vec![(355.5, 36.5), (196.16666666666663, 111.83333333333334), (97.99999999999997, 248.66666666666663), (94.0, 418.0)],
            ]
        );
    }
}
