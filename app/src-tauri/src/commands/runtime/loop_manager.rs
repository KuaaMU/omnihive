use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use crate::engine::{extract, state, state_machine, task_model, checkpoint};
use crate::engine::state_machine::{TaskStatus, TaskEvent};
use crate::engine::task_model::{Task, Step};
use crate::engine::checkpoint::Checkpoint;
use crate::models::CycleResult;
use super::credentials::ApiCredentials;
use super::cycle_executor::{run_api_cycle, load_cycle_history, save_cycle_history};
use super::status::emit_project_event;

// ===== Running Loop Registry =====

pub(crate) static RUNNING_LOOPS: std::sync::LazyLock<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

// ===== Start Loop =====

pub(crate) fn start_loop_impl(
    dir: PathBuf,
    project_dir: String,
    credentials: ApiCredentials,
    agent_roles: Vec<String>,
    loop_interval: u32,
    cycle_timeout: u32,
    max_errors: u32,
) -> Result<bool, String> {
    {
        let loops = RUNNING_LOOPS.lock().map_err(|e| e.to_string())?;
        if let Some(flag) = loops.get(&project_dir) {
            if !flag.load(Ordering::Relaxed) {
                return Err("Loop is already running for this project".to_string());
            }
        }
    }

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop_flag);

    {
        let mut loops = RUNNING_LOOPS.lock().map_err(|e| e.to_string())?;
        loops.insert(project_dir.clone(), Arc::clone(&stop_flag));
    }

    let project_dir_clone = project_dir.clone();
    thread::spawn(move || {
        run_loop(
            dir,
            project_dir_clone,
            credentials,
            agent_roles,
            loop_interval,
            cycle_timeout,
            max_errors,
            stop_clone,
        );
    });

    Ok(true)
}

// ===== Stop Loop =====

pub(crate) fn stop_loop_impl(project_dir: &str, dir: &PathBuf) -> Result<bool, String> {
    let stopped = {
        let loops = RUNNING_LOOPS.lock().map_err(|e| e.to_string())?;
        if let Some(flag) = loops.get(project_dir) {
            flag.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    };

    if stopped {
        state::append_log(dir, "Stop signal sent by user");
        Ok(true)
    } else {
        state::write_state(dir, "stopped", 0, 0, 0).ok();
        Ok(false)
    }
}

// ===== Background Loop =====

#[tracing::instrument(skip(credentials, stop_flag), fields(agents = %agent_roles.len()))]
fn run_loop(
    dir: PathBuf,
    project_dir: String,
    credentials: ApiCredentials,
    agent_roles: Vec<String>,
    loop_interval: u32,
    cycle_timeout: u32,
    max_errors: u32,
    stop_flag: Arc<AtomicBool>,
) {
    // Create a Task for this loop execution
    let goal = format!("Autonomous loop for {}", project_dir);
    let task = Task::new(&project_dir, &goal, agent_roles.clone());
    let task = task.with_status(
        state_machine::transition(task.status, TaskEvent::PlanComplete)
            .unwrap_or(TaskStatus::Running),
    );

    let task_id = task.task_id.clone();
    let trace_id = task.trace_id.clone();

    // Write initial task state (JSON format)
    task_model::write_task_state(&dir, &task).ok();

    let _task_span = tracing::info_span!(
        "task",
        task_id = %task_id,
        trace_id = %trace_id,
    )
    .entered();

    tracing::info!(
        agents = %agent_roles.len(),
        interval = loop_interval,
        timeout = cycle_timeout,
        "Task started"
    );

    let mut current_task = task;
    let mut history: Vec<CycleResult> = load_cycle_history(&dir);

    state::append_log(
        &dir,
        &format!(
            "Loop started [task={}] | {} agents: [{}] | interval={}s timeout={}s max_errors={}",
            &task_id[..8],
            agent_roles.len(),
            agent_roles.join(", "),
            loop_interval,
            cycle_timeout,
            max_errors,
        ),
    );

    // Also maintain legacy state for backward compat
    state::write_state(&dir, "running", 0, 0, 0).ok();

    let mut cycle: u32 = 0;

    loop {
        if stop_flag.load(Ordering::Relaxed) {
            state::append_log(&dir, "Loop stopped by user");
            current_task = current_task.with_status(
                state_machine::transition(current_task.status, TaskEvent::UserCancel)
                    .unwrap_or(TaskStatus::Cancelled),
            );
            task_model::write_task_state(&dir, &current_task).ok();
            state::write_state(&dir, "stopped", cycle, cycle, current_task.consecutive_errors).ok();
            break;
        }

        cycle += 1;
        let agent_idx = ((cycle - 1) as usize) % agent_roles.len();
        let current_agent = &agent_roles[agent_idx];

        // Create a Step for this cycle
        let step = Step::new(&task_id, current_agent);
        let step_id = step.step_id.clone();

        let _step_span = tracing::info_span!(
            "step",
            step_id = %step_id,
            agent = %current_agent,
            cycle = %cycle,
        )
        .entered();

        state::append_log(
            &dir,
            &format!("=== Cycle {} | Agent: {} | step={} ===", cycle, current_agent, &step_id[..8]),
        );

        let started_at = chrono::Local::now().format("%+").to_string();
        state::write_state(&dir, "running", cycle, cycle, current_task.consecutive_errors).ok();

        let result = run_api_cycle(
            &dir,
            &project_dir,
            &credentials,
            current_agent,
            cycle,
            cycle_timeout,
        );

        let completed_at = chrono::Local::now().format("%+").to_string();

        match result {
            Ok((output, input_tokens, output_tokens)) => {
                let preview = extract::truncate_string(&output, 200);

                // Update step (immutable)
                let completed_step = step.completed(input_tokens, output_tokens, &preview);

                tracing::info!(
                    input_tokens = input_tokens,
                    output_tokens = output_tokens,
                    "Step completed"
                );

                state::append_log(
                    &dir,
                    &format!(
                        "Cycle {} completed | Tokens: {}in/{}out | Output: {}",
                        cycle, input_tokens, output_tokens, preview
                    ),
                );

                emit_project_event(
                    &project_dir,
                    "cycle_complete",
                    current_agent,
                    &format!("Cycle {} completed ({}+{} tokens)", cycle, input_tokens, output_tokens),
                    &preview,
                );

                // Update task (immutable)
                current_task = current_task.with_step_completed(&completed_step.step_id);

                // Backward compat: also push to CycleResult history
                history.push(CycleResult {
                    cycle_number: cycle,
                    started_at,
                    completed_at,
                    agent_role: current_agent.clone(),
                    action: format!("{} analysis ({}+{} tokens)", current_agent, input_tokens, output_tokens),
                    outcome: preview,
                    files_changed: vec![],
                    error: None,
                });
            }
            Err(err) => {
                let _failed_step = step.failed(&err);

                tracing::warn!(error = %err, "Step failed");

                state::append_log(
                    &dir,
                    &format!(
                        "ERROR: Cycle {} failed: {} (consecutive: {})",
                        cycle, err, current_task.consecutive_errors + 1
                    ),
                );

                emit_project_event(
                    &project_dir,
                    "cycle_error",
                    current_agent,
                    &format!("Cycle {} failed (error {})", cycle, current_task.consecutive_errors + 1),
                    &extract::truncate_string(&err, 200),
                );

                current_task = current_task.with_error(&err);

                history.push(CycleResult {
                    cycle_number: cycle,
                    started_at,
                    completed_at,
                    agent_role: current_agent.clone(),
                    action: format!("Attempted {} agent cycle", current_agent),
                    outcome: String::new(),
                    files_changed: vec![],
                    error: Some(err),
                });

                if current_task.consecutive_errors >= max_errors {
                    state::append_log(
                        &dir,
                        &format!(
                            "FATAL: Max consecutive errors ({}) reached. Stopping loop.",
                            max_errors
                        ),
                    );
                    current_task = current_task.with_status(
                        state_machine::transition(current_task.status, TaskEvent::MaxRetriesExceeded)
                            .unwrap_or(TaskStatus::Failed),
                    );
                    task_model::write_task_state(&dir, &current_task).ok();
                    state::write_state(&dir, "error", cycle, cycle, current_task.consecutive_errors).ok();
                    save_cycle_history(&dir, &history);
                    cleanup_loop(&project_dir);
                    return;
                }
            }
        }

        // Persist state after each cycle
        task_model::write_task_state(&dir, &current_task).ok();
        state::write_state(&dir, "running", cycle, cycle, current_task.consecutive_errors).ok();
        save_cycle_history(&dir, &history);

        // Save checkpoint for crash recovery
        let consensus = std::fs::read_to_string(dir.join("memories/consensus.md"))
            .unwrap_or_default();
        let cp = Checkpoint::from_task(&current_task, &consensus);
        checkpoint::save_checkpoint(&dir, &cp).ok();

        sleep_with_stop_check(loop_interval, &stop_flag);
    }

    cleanup_loop(&project_dir);
}

// ===== Helpers =====

fn cleanup_loop(project_dir: &str) {
    if let Ok(mut loops) = RUNNING_LOOPS.lock() {
        loops.remove(project_dir);
    }
}

fn sleep_with_stop_check(seconds: u32, stop_flag: &Arc<AtomicBool>) {
    let total = Duration::from_secs(seconds as u64);
    let check = Duration::from_secs(1);
    let mut elapsed = Duration::ZERO;
    while elapsed < total {
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }
        thread::sleep(check);
        elapsed += check;
    }
}
