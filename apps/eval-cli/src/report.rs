use comfy_table::{presets::UTF8_FULL_CONDENSED, Cell, Color, ContentArrangement, Table};

use hypr_eval::EvalResult;

pub fn render_json(results: &[EvalResult]) -> std::result::Result<(), String> {
    let json = serde_json::to_string_pretty(
        &results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "case_id": r.case_id,
                    "model": r.model,
                    "output": r.output,
                    "scores": r.scores.iter().map(|s| {
                        serde_json::json!({
                            "rubric_name": s.rubric_name,
                            "passed": s.passed,
                            "value": s.value,
                            "reasoning": s.reasoning,
                            "grader_type": s.grader_type,
                            "grader_model": s.grader_model,
                            "pass_rate": s.pass_rate,
                            "samples": s.samples,
                        })
                    }).collect::<Vec<_>>(),
                    "error": r.error,
                    "generation_id": r.generation_id,
                    "usage": {
                        "prompt_tokens": r.usage.prompt_tokens,
                        "completion_tokens": r.usage.completion_tokens,
                        "total_tokens": r.usage.total_tokens,
                        "cost": r.usage.cost,
                    }
                })
            })
            .collect::<Vec<_>>(),
    )
    .map_err(|e| format!("Failed to encode JSON: {}", e))?;

    println!("{}", json);

    for r in results {
        if r.error.is_some() {
            return Err("evaluation failed".to_string());
        }
    }

    Ok(())
}

pub fn render_results(results: &[EvalResult]) -> std::result::Result<(), String> {
    let rubric_names = extract_rubric_names(results);

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic);

    let mut header = vec!["Model".to_string()];
    header.extend(rubric_names.iter().cloned());
    header.push("Total".to_string());
    header.push("Cost".to_string());
    table.set_header(header);

    let mut totals: Vec<i32> = vec![0; rubric_names.len()];
    let mut grand_total = 0;
    let mut max_total = 0;
    let mut total_failed = 0;
    let mut total_cost = 0.0;
    let mut error_details: Vec<String> = Vec::new();

    for r in results {
        let mut row: Vec<Cell> = vec![Cell::new(&r.model)];

        if let Some(ref err) = r.error {
            total_failed += 1;
            for _ in &rubric_names {
                row.push(Cell::new("-"));
            }
            row.push(Cell::new("error").fg(Color::Red));
            row.push(Cell::new("-"));
            table.add_row(row);
            error_details.push(format!("{}: {}", r.model, err));
            continue;
        }

        let (passed, total) = r.tally_score();
        max_total += total;
        total_cost += r.usage.cost;

        for (i, s) in r.scores.iter().enumerate() {
            if s.passed {
                totals[i] += 1;
                grand_total += 1;
                row.push(Cell::new("1"));
            } else {
                row.push(Cell::new("0").fg(Color::Red));
            }
        }

        if passed == total {
            row.push(Cell::new(format!("{}/{}", passed, total)));
        } else {
            row.push(Cell::new(format!("{}/{}", passed, total)).fg(Color::Red));
        }

        row.push(Cell::new(format_cost(r.usage.cost)));
        table.add_row(row);
    }

    let mut footer: Vec<Cell> = vec![Cell::new("Total")];
    for total in &totals {
        footer.push(Cell::new(format!("{}", total)));
    }
    footer.push(Cell::new(format!("{}/{}", grand_total, max_total)));
    footer.push(Cell::new(format_cost(total_cost)));
    table.add_row(footer);

    println!("{}", table);

    if !error_details.is_empty() {
        eprintln!();
        eprintln!("\x1b[31mErrors:\x1b[0m");
        for detail in &error_details {
            eprintln!("\x1b[31m  - {}\x1b[0m", detail);
        }
    }

    if total_failed > 0 {
        return Err("evaluation failed".to_string());
    }

    Ok(())
}

fn extract_rubric_names(results: &[EvalResult]) -> Vec<String> {
    for r in results {
        if r.error.is_none() && !r.scores.is_empty() {
            return r.scores.iter().map(|s| s.rubric_name.clone()).collect();
        }
    }
    Vec::new()
}

fn format_cost(cost: f64) -> String {
    if cost == 0.0 {
        return "-".to_string();
    }
    if cost < 0.01 {
        format!("{:.6}", cost)
    } else {
        format!("{:.4}", cost)
    }
}
