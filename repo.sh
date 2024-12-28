#!/bin/bash
urls=("git@github.com:reL4team2/sel4_common.git,sel4_common"
  "git@github.com:reL4team2/sel4_task.git,sel4_task"
  "git@github.com:reL4team2/sel4_ipc.git,sel4_ipc"
  "git@github.com:reL4team2/sel4_vspace.git,sel4_vspace"
  "git@github.com:reL4team2/sel4_cspace.git,sel4_cspace"
  "git@github.com:reL4team2/rel4_kernel.git,kernel"
  "git@github.com:reL4team2/driver-collect.git,driver-collect"
  "git@github.com:reL4team2/serial-impl-pl011.git,serial-impl/pl011"
  "git@github.com:reL4team2/serial-impl-sbi.git,serial-impl/sbi"
  "git@github.com:reL4team2/serial-frame.git,serial-frame"
)
branch=master

for str in ${urls[@]}; do
  IFS=',' read -r -a inner_array <<<"$str"
  echo ${inner_array[0]}
  git subrepo clone ${inner_array[0]} -b $branch ${inner_array[1]}
done

cp -r kernel/.cargo .cargo 
cp -r kernel/build.py build.py
cp -r kernel/Cargo.toml.virtual Cargo.toml