cmake_minimum_required(VERSION 3.7.2)
project(ReL4Kernel)

set(RUST_TARGET riscv64imac-unknown-none-elf)

if(KernelArchRiscV)
    set(KERNEL_ELF_PATH ${PROJECT_SOURCE_DIR}/target/${RUST_TARGET}/release/rel4_kernel)
endif()

set(BUILD_COMMAND cargo build --target ${RUST_TARGET} --release --features BUILD_BINARY)
if(KernelIsMCS)
    set(BUILD_COMMAND ${BUILD_COMMAND} --features KERNEL_MCS)
endif()

add_custom_command(
    OUTPUT ${CMAKE_BINARY_DIR}/reL4/kernel.elf
    COMMAND cargo clean
    COMMAND cargo update -p home --precise 0.5.5
    COMMAND ${BUILD_COMMAND} 
    COMMAND ${CMAKE_OBJCOPY} --remove-section=.riscv.attributes ${KERNEL_ELF_PATH} ${CMAKE_BINARY_DIR}/reL4/kernel.elf
    WORKING_DIRECTORY ${PROJECT_SOURCE_DIR}
    COMMENT "Build and prepare reL4 kernel.elf"
)

add_custom_target(
    build_reL4
    DEPENDS ${CMAKE_BINARY_DIR}/reL4/kernel.elf
)

add_executable(reL4_kernel.elf IMPORTED GLOBAL)
set_target_properties(reL4_kernel.elf PROPERTIES
    IMPORTED_LOCATION ${CMAKE_BINARY_DIR}/reL4/kernel.elf
)