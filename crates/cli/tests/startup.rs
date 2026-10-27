use std::process::Command;

#[test]
fn cli_starts_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_pipeline-autopsy"))
        .output()
        .expect("CLI should launch");

    assert!(output.status.success(), "CLI failed: {output:?}");
    assert!(output.stderr.is_empty(), "unexpected stderr: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("CLI output should be UTF-8");
    assert!(stdout.contains("Pipeline Autopsy"));
    assert!(stdout.contains(pipeline_autopsy_core::VERSION));
}
