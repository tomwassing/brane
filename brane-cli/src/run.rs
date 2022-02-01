use crate::{docker::DockerExecutor, packages};
use anyhow::Result;
use brane_bvm::vm::Vm;
use brane_dsl::{Compiler, CompilerOptions, Lang};
use std::fs;
use std::path::PathBuf;

///
///
///
pub async fn handle(
    file: PathBuf,
    data: Option<PathBuf>,
) -> Result<()> {
    let source_code = fs::read_to_string(&file)?;

    let compiler_options = CompilerOptions::new(Lang::BraneScript);
    let package_index = packages::get_package_index()?;
    let mut compiler = Compiler::new(compiler_options, package_index.clone());

    let executor = DockerExecutor::new(data);
    let mut vm = match Vm::new_with(executor, Some(package_index), None) {
        Ok(vm)      => vm,
        Err(reason) => { eprintln!("Could not create VM: {}", reason); return Ok(()); }
    };

    match compiler.compile(source_code) {
        /* TIM */
        // Ok(function) => vm.main(function).await,
        Ok(function) => {
            if let Err(reason) = vm.main(function).await {
                eprintln!("{}", reason);
            }
        }
        /*******/
        Err(error) => eprintln!("{:?}", error),
    }

    Ok(())
}
