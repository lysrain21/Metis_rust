use crate::core::constraints::{OptimizationAlgorithm, RoutingConstraints, SplitConfig};
use crate::core::graph::RoutingGraph;
use crate::core::pool::Edge;

const GOLDEN_RATIO: f64 = 1.618_033_988_749_894_8;
const RESPHI: f64 = 2.0 - GOLDEN_RATIO;
const MIN_TOL: f64 = 1e-9;
const MAX_ITER: usize = 64;

#[derive(Clone, Copy)]
enum SearchMethod {
    Golden,
    Brent,
}

fn normalize_allocations(allocations: &mut [f64], total: f64) {
    let mut sum: f64 = allocations.iter().sum();
    if !sum.is_finite() || sum <= MIN_TOL {
        if let Some(first) = allocations.first_mut() {
            *first = total.max(0.0);
            for value in allocations.iter_mut().skip(1) {
                *value = 0.0;
            }
        }
        return;
    }

    let scale = if sum.abs() < MIN_TOL {
        0.0
    } else {
        total / sum
    };

    for value in allocations.iter_mut() {
        *value = (*value * scale).max(0.0);
    }

    sum = allocations.iter().sum();
    if (sum - total).abs() > MIN_TOL && sum > MIN_TOL {
        let correction = (total - sum) / allocations.len().max(1) as f64;
        for value in allocations.iter_mut() {
            *value = (*value + correction).max(0.0);
        }
    }
}

fn initial_allocations(amount_in: f64, routes: usize) -> Vec<f64> {
    if routes == 0 {
        return Vec::new();
    }
    let share = amount_in.max(0.0) / routes as f64;
    vec![share; routes]
}

fn simulate_total_out(rg: &RoutingGraph, candidates: &[Vec<Edge>], allocations: &[f64]) -> f64 {
    let mut tmp_graph = rg.clone();
    let mut total_out = 0.0;

    for (path, &amount) in candidates.iter().zip(allocations.iter()) {
        if amount <= 0.0 {
            continue;
        }
        let mut current = amount;
        let mut valid = true;
        for edge in path {
            match tmp_graph.pools.get_mut(&edge.pool_id) {
                Some(pool) => {
                    current = pool.apply_virtual_fill(current);
                }
                None => {
                    valid = false;
                    break;
                }
            }
        }
        if !valid {
            return f64::NEG_INFINITY;
        }
        total_out += current;
    }

    total_out
}

fn tolerance_amount(config: &SplitConfig, amount_in: f64, routes: usize) -> f64 {
    config.tolerance_amount(amount_in, routes).max(MIN_TOL)
}

fn golden_section_search<F>(mut a: f64, mut b: f64, tol: f64, f: &F) -> (f64, f64)
where
    F: Fn(f64) -> f64,
{
    if (b - a).abs() < tol {
        let mid = (a + b) / 2.0;
        return (mid, f(mid));
    }

    let mut c = b - (b - a) * RESPHI;
    let mut d = a + (b - a) * RESPHI;
    let mut fc = f(c);
    let mut fd = f(d);

    for _ in 0..MAX_ITER {
        if (b - a).abs() <= tol {
            break;
        }

        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - (b - a) * RESPHI;
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + (b - a) * RESPHI;
            fd = f(d);
        }
    }

    if fc < fd {
        (c, fc)
    } else {
        (d, fd)
    }
}

#[allow(unused_assignments)]
fn brent_search<F>(mut a: f64, mut b: f64, tol: f64, f: &F) -> (f64, f64)
where
    F: Fn(f64) -> f64,
{
    let mut x = a + 0.5 * (b - a);
    let mut w = x;
    let mut v = x;
    let mut fx = f(x);
    let mut fw = fx;
    let mut fv = fx;
    let mut d: f64 = 0.0;
    let mut e: f64 = 0.0;

    for _ in 0..MAX_ITER {
        let m = 0.5 * (a + b);
        let tol1 = tol * x.abs() + MIN_TOL;
        let tol2 = 2.0 * tol1;

        if (x - m).abs() <= tol2 - 0.5 * (b - a) {
            break;
        }

        if e.abs() > tol1 {
            let r = (x - w) * (fx - fv);
            let mut q = (x - v) * (fx - fw);
            let mut p = (x - v) * q - (x - w) * r;
            q = 2.0 * (q - r);
            if q > 0.0 {
                p = -p;
            } else {
                q = -q;
            }
            if q.abs() > MIN_TOL {
                let d_temp = p / q;
                if (d_temp > a - x + tol2) && (d_temp < b - x - tol2) {
                    d = d_temp;
                } else {
                    e = if x < m { b - x } else { a - x };
                    d = 0.381_966_011_250_105_1 * e;
                }
            } else {
                e = if x < m { b - x } else { a - x };
                d = 0.381_966_011_250_105_1 * e;
            }
        } else {
            e = if x < m { b - x } else { a - x };
            d = 0.381_966_011_250_105_1 * e;
        }

        let u = if d.abs() >= tol1 {
            x + d
        } else {
            x + if d > 0.0 { tol1 } else { -tol1 }
        };
        let fu = f(u);

        if fu <= fx {
            if u < x {
                b = x;
            } else {
                a = x;
            }
            v = w;
            fv = fw;
            w = x;
            fw = fx;
            x = u;
            fx = fu;
        } else {
            if u < x {
                a = u;
            } else {
                b = u;
            }
            if fu <= fw || w == x {
                v = w;
                fv = fw;
                w = u;
                fw = fu;
            } else if fu <= fv || v == x || v == w {
                v = u;
                fv = fu;
            }
        }
    }

    (x, fx)
}

fn coordinate_descent(
    method: SearchMethod,
    rg: &RoutingGraph,
    candidates: &[Vec<Edge>],
    amount_in: f64,
    config: &SplitConfig,
) -> Vec<f64> {
    let routes = candidates.len();
    if routes == 0 || amount_in <= 0.0 {
        return vec![0.0; routes];
    }

    let mut allocations = initial_allocations(amount_in, routes);
    normalize_allocations(&mut allocations, amount_in);

    let tolerance = tolerance_amount(config, amount_in, routes);
    let max_outer = config.max_iterations().min(MAX_ITER);

    for _ in 0..max_outer {
        let mut improved = false;
        let total_amount = amount_in;
        for i in 0..routes {
            let lower = 0.0_f64;
            let upper = total_amount;
            if upper <= lower + tolerance {
                continue;
            }

            let search = |x: f64| {
                let mut trial = allocations.clone();
                trial[i] = x.max(0.0);
                normalize_allocations(&mut trial, total_amount);
                -simulate_total_out(rg, candidates, &trial)
            };

            let (best_x, best_eval) = match method {
                SearchMethod::Golden => golden_section_search(lower, upper, tolerance, &search),
                SearchMethod::Brent => brent_search(lower, upper, tolerance, &search),
            };

            if best_eval.is_infinite() {
                continue;
            }

            if (best_x - allocations[i]).abs() > tolerance {
                allocations[i] = best_x.max(0.0);
                normalize_allocations(&mut allocations, total_amount);
                improved = true;
            }
        }
        if !improved {
            break;
        }
    }

    normalize_allocations(&mut allocations, amount_in);
    allocations
}

pub fn run_golden_section(
    rg: &RoutingGraph,
    candidates: &[Vec<Edge>],
    amount_in: f64,
    constraints: &RoutingConstraints,
) -> Vec<f64> {
    let config = constraints.split_config.clone().unwrap_or_default();
    coordinate_descent(SearchMethod::Golden, rg, candidates, amount_in, &config)
}

pub fn run_brent(
    rg: &RoutingGraph,
    candidates: &[Vec<Edge>],
    amount_in: f64,
    constraints: &RoutingConstraints,
) -> Vec<f64> {
    let config = constraints.split_config.clone().unwrap_or_default();
    coordinate_descent(SearchMethod::Brent, rg, candidates, amount_in, &config)
}

pub fn run_hybrid(
    rg: &RoutingGraph,
    candidates: &[Vec<Edge>],
    amount_in: f64,
    constraints: &RoutingConstraints,
) -> (Vec<f64>, OptimizationAlgorithm) {
    let golden_allocs = run_golden_section(rg, candidates, amount_in, constraints);
    let golden_score = simulate_total_out(rg, candidates, &golden_allocs);

    let brent_allocs = run_brent(rg, candidates, amount_in, constraints);
    let brent_score = simulate_total_out(rg, candidates, &brent_allocs);

    if brent_score.is_finite() && brent_score > golden_score {
        (brent_allocs, OptimizationAlgorithm::Brent)
    } else {
        (golden_allocs, OptimizationAlgorithm::GoldenSection)
    }
}

pub fn evaluate_allocation(
    rg: &RoutingGraph,
    candidates: &[Vec<Edge>],
    allocations: &[f64],
) -> f64 {
    simulate_total_out(rg, candidates, allocations)
}
