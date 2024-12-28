# 使用Subrepo大小仓库同步开发reL4指南
大小仓库的开发须知请看：
[.github/profile/rel4多仓开发提案.md at main · kern-crates/.github](https://github.com/kern-crates/.github/blob/main/profile/rel4%E5%A4%9A%E4%BB%93%E5%BC%80%E5%8F%91%E6%8F%90%E6%A1%88.md)
当前大仓库位于:[https://github.com/reL4team2/rel4-integral](https://github.com/reL4team2/rel4-integral) 包含了众多的小仓库信息
包含的小仓库有：
```
urls=("git@github.com:reL4team2/sel4_common.git"
    "git@github.com:reL4team2/sel4_task.git"
    "git@github.com:reL4team2/sel4_ipc.git "
    "git@github.com:reL4team2/sel4_vspace.git"
    "git@github.com:reL4team2/sel4_cspace.git"
    "git@github.com:reL4team2/rel4_kernel.git"
    "git@github.com:reL4team2/driver-collect.git"
    "git@github.com:reL4team2/serial-impl-pl011.git"
    "git@github.com:reL4team2/serial-impl-sbi.git"
    "git@github.com:reL4team2/serial-frame.git"
)
```
大仓库中的全部更新，会通过CI自动将相关的commit信息更新到小仓库当中，反过来，小仓库中的任何更新也会在大仓库中进行同步。
接下来分别介绍从大仓库 和 小仓库端的更新方法
### 代码提交到大仓库的方法举例说明
依次执行以下指令：

- `git pull #因为大小仓库任意一端都有可能更新，每次开发前需要保证大仓库中的内容是最新的 `
- 完成修改后，使用`git add . && git commit -m "COMMIT_MESSAGE" && git push`将修改push 到远端分支。
   - 此时在History中应该能看到类似的两条commit信息，第一条commit信息是我们自己的包含COMMIT_MESSAGE的信息，是我们自己的修改，会进行五项CI测试，包含aarch riscv平台的kernel build 、sel4test运行以及大小仓库的同步CI，第二条COMMIT信息则是git subrepo +COMMIT_MESSAGE，是大仓库记录和小仓库对应commit版本的对应信息，只更新.gitsubrepo中的commit（小仓库中最新的commit id）和parent（大仓库中COMMIT_MESSAGE对应的commit id）两项内容。![image.png](https://cdn.nlark.com/yuque/0/2024/png/2964023/1724037308030-325d7e3d-bb35-43cb-9fa5-c30d7311bcee.png#averageHue=%23fefefe&clientId=uab018d93-c46d-4&from=paste&height=175&id=u45e7f70a&originHeight=350&originWidth=3193&originalType=binary&ratio=2&rotation=0&showTitle=false&size=86143&status=done&style=none&taskId=u8957cc5c-38c6-4535-b1cf-5050627c0c4&title=&width=1596.5)
- 最后需要`git pull`将第二次commit修改pull下拉，从而完成一次修改。

相较于普通的git使用来说，仅增加了最后的 git pull 一个步骤。
### 代码提交到小仓库的方法
正常使用git即可，没有什么特别。
### 注意事项

1.  大仓库每次push之后必须要跟一次pull，否则commit记录会乱掉
2.  小仓库每次push的最新commit信息应该保证不与大仓库最新commit信息存在真子集的关系（谁是谁的真子集均不被允许），如一方是“Hello World”，一方是“Hello”。
3. 大仓库commit信息应该不包含"git subrepo"这一子字符串，CI会检测此字符串来选择是否进行push到小仓库中。
## 从仓库中分离文件夹作为单独仓库的方法
当从大仓库分离文件夹进入子仓库时，希望修改对应的文件夹的log可以保留，可以使用如下方法：
以polyhal的example文件夹为例，polyhal的commit log信息如下：
![image.png](https://cdn.nlark.com/yuque/0/2024/png/2964023/1724905895524-55eca1fa-689d-48d5-8e31-fba97285f3fb.png#averageHue=%23fefefe&clientId=u50755ceb-5e8e-4&from=paste&height=767&id=u91778817&originHeight=1533&originWidth=2157&originalType=binary&ratio=2&rotation=0&showTitle=false&size=216425&status=done&style=none&taskId=uaffc7ea9-09a9-4292-8052-d4e1cb321c3&title=&width=1078.5)
其中与example文件夹相关的是8.5号的feat:release 0.1.3 version commit记录：
![image.png](https://cdn.nlark.com/yuque/0/2024/png/2964023/1724905954966-f92081fa-6ba6-4b67-8555-3c1140b0f740.png#averageHue=%23dabd9e&clientId=u50755ceb-5e8e-4&from=paste&height=364&id=ue630f5c4&originHeight=728&originWidth=2587&originalType=binary&ratio=2&rotation=0&showTitle=false&size=91011&status=done&style=none&taskId=u7c62a80d-19c8-4b0f-b655-74b406ea76c&title=&width=1293.5)
使用如下命令：
```bash
git subtree split -P example -b example  # -P path -b branch
```
就会创建一个新的分支example，其中的文件即为example中的文件，commit记录也为example文件夹的修改记录：
![image.png](https://cdn.nlark.com/yuque/0/2024/png/2964023/1724906091780-03af445b-ac10-4686-8b5c-a9882f607f7f.png#averageHue=%23300a24&clientId=u50755ceb-5e8e-4&from=paste&height=460&id=u24fab75d&originHeight=919&originWidth=1998&originalType=binary&ratio=2&rotation=0&showTitle=false&size=285767&status=done&style=none&taskId=u5fc9ba79-1afa-4bce-9441-02d98e1439c&title=&width=999)
