// 仅复用本仓库 main 上成功的 Release 构建，避免把其他任务或提交的附件发布出去。
function selectBuild(run, artifacts, repository) {
  if (
    run.repository?.full_name !== repository ||
    run.head_repository?.full_name !== repository ||
    run.path !== '.github/workflows/release.yml' ||
    run.event !== 'workflow_dispatch' ||
    run.head_branch !== 'main' ||
    run.status !== 'completed' ||
    run.conclusion !== 'success'
  ) {
    throw new Error('仅支持本仓库 main 上已成功完成的手动 Release 构建');
  }
  if (!Number.isSafeInteger(run.id) || run.id <= 0 || !/^[0-9a-f]{40}$/.test(run.head_sha)) {
    throw new Error('构建编号或来源提交无效');
  }

  const expected = new Set(['honk-core-aarch64-unknown-linux-musl-full', 'openwrt-25.12-packages']);
  const selected = artifacts.filter((artifact) => expected.has(artifact.name));
  if (selected.length !== expected.size || new Set(selected.map((artifact) => artifact.name)).size !== expected.size) {
    throw new Error('需要完整的 ARM64 musl full 归档和 OpenWrt 安装包；缺少或重复产物时不能发布');
  }
  for (const artifact of selected) {
    if (
      artifact.expired !== false ||
      !Number.isSafeInteger(artifact.size_in_bytes) || artifact.size_in_bytes <= 0 ||
      !Number.isSafeInteger(artifact.id) || artifact.id <= 0 ||
      artifact.workflow_run?.id !== run.id ||
      artifact.workflow_run?.head_sha !== run.head_sha ||
      artifact.workflow_run?.repository_id !== run.repository.id ||
      artifact.workflow_run?.head_repository_id !== run.repository.id
    ) {
      throw new Error(`产物已过期、为空或来源不匹配：${artifact.name}；请重新手动构建`);
    }
  }
  return {
    runId: String(run.id),
    sha: run.head_sha,
    tag: `debug.build.${run.id}`,
    artifactIds: selected.map((artifact) => artifact.id).join(','),
  };
}

// GITHUB_TOKEN 无法为工作流文件不同的历史提交建标签，维护者须先创建它。
async function verifyReleaseTag(github, repository, selected) {
  let ref;
  try {
    ({ data: ref } = await github.rest.git.getRef({ ...repository, ref: `tags/${selected.tag}` }));
  } catch (error) {
    if (error.status !== 404) throw error;
    throw new Error([
      `缺少发布标签 ${selected.tag}。GITHUB_TOKEN 不能为工作流文件不同的历史提交创建标签。`,
      '请维护者在仓库目录执行以下命令，再从 main 重新运行 Publish existing build：',
      `git tag ${selected.tag} ${selected.sha}`,
      `git push origin refs/tags/${selected.tag}`,
    ].join('\n'), { cause: error });
  }
  let object = ref.object;
  for (let depth = 0; object.type === 'tag' && depth < 8; depth++) {
    const { data } = await github.rest.git.getTag({ ...repository, tag_sha: object.sha });
    object = data.object;
  }
  if (object.type !== 'commit' || object.sha !== selected.sha) {
    throw new Error(`发布标签 ${selected.tag} 未指向原构建提交 ${selected.sha}，拒绝发布或移动标签`);
  }
}

module.exports = { selectBuild, verifyReleaseTag };
