set(REL4_PATH "${CMAKE_CURRENT_LIST_DIR}" CACHE STRING "")

macro(rel4_import_kernel)
    add_subdirectory(${REL4_PATH} ${CMAKE_BINARY_DIR}/reL4)
endmacro()