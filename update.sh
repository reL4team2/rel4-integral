#!/bin/bash
REPOs=("sel4_common" "sel4_task" "sel4_ipc" "sel4_vspace" "sel4_cspace" "kernel" "driver-collect" serial-impl/pl011 serial-impl/sbi serial-frame)
PARENT_COMMIT_ID=$(git log -1 --pretty=%H | head -n 1)
echo $PARENT_COMMIT_ID

# urls=("git@github.com:reL4team2/sel4_common.git"
#     "git@github.com:reL4team2/sel4_task.git"
#     "git@github.com:reL4team2/sel4_ipc.git "
#     "git@github.com:reL4team2/sel4_vspace.git"
#     "git@github.com:reL4team2/sel4_cspace.git"
#     "git@github.com:reL4team2/rel4_kernel.git"
#     "git@github.com:reL4team2/driver-collect.git"
#     "git@github.com:reL4team2/serial-impl-pl011.git"
#     "git@github.com:reL4team2/serial-impl-sbi.git"
#     "git@github.com:reL4team2/serial-frame.git"
# )

# for url in ${urls[@]}; do
#     git clone $url
# done

for repo in ${REPOs[@]}; do
    sed -i "10c\ \tparent = $PARENT_COMMIT_ID" $repo/.gitrepo
done
