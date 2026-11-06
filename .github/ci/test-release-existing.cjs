const assert = require('node:assert/strict');
const test = require('node:test');
const { selectBuild, verifyReleaseTag } = require('./release-existing.cjs');

const repository = 'example/honk';
function fixture() {
  const run = {
    id: 42221567511,
    repository: { id: 100, full_name: repository },
    head_repository: { full_name: repository },
    path: '.github/workflows/release.yml',
    event: 'workflow_dispatch',
    head_branch: 'main',
    head_sha: '6137d9b3461e54b6870597799dcfa2b94c2c03e3',
    status: 'completed',
    conclusion: 'success',
  };
  const names = ['honk-core-aarch64-unknown-linux-musl-full', 'openwrt-25.12-packages'];
  const artifacts = names.map((name, index) => ({
    id: index + 1, name, size_in_bytes: 100, expired: false,
    workflow_run: { id: run.id, head_sha: run.head_sha, repository_id: 100, head_repository_id: 100 },
  }));
  return { run, artifacts };
}

test('成功的 main 构建保留原提交并使用固定编号发布', () => {
  const { run, artifacts } = fixture();
  const selected = selectBuild(run, artifacts, repository);
  assert.equal(selected.sha, run.head_sha);
  assert.equal(selected.tag, 'debug.build.42221567511');
  assert.equal(selected.artifactIds, '1,2');
});

test('发布只读校验指向原构建的轻量标签和附注标签', async () => {
  const { run, artifacts } = fixture();
  const selected = selectBuild(run, artifacts, repository);
  const repo = { owner: 'example', repo: 'honk' };
  for (const annotated of [false, true]) {
    const github = { rest: { git: {
      getRef: async (request) => {
        assert.deepEqual(request, { ...repo, ref: `tags/${selected.tag}` });
        return { data: { object: annotated
          ? { type: 'tag', sha: 'a'.repeat(40) }
          : { type: 'commit', sha: selected.sha } } };
      },
      getTag: async (request) => {
        assert.equal(annotated, true);
        assert.deepEqual(request, { ...repo, tag_sha: 'a'.repeat(40) });
        return { data: { object: { type: 'commit', sha: selected.sha } } };
      },
    } } };
    await verifyReleaseTag(github, repo, selected);
  }
});

test('缺少历史标签时先给出维护者命令，不尝试创建标签', async () => {
  const { run, artifacts } = fixture();
  const selected = selectBuild(run, artifacts, repository);
  const github = { rest: { git: {
    getRef: async () => { throw Object.assign(new Error('Not Found'), { status: 404 }); },
  } } };
  await assert.rejects(verifyReleaseTag(github, { owner: 'example', repo: 'honk' }, selected), (error) => {
    assert.ok(error.message.includes(`git tag ${selected.tag} ${selected.sha}`));
    assert.ok(error.message.includes(`git push origin refs/tags/${selected.tag}`));
    assert.ok(error.message.includes('GITHUB_TOKEN'));
    return true;
  });
});

test('标签指向其他提交或非提交对象时拒绝发布', async () => {
  const { run, artifacts } = fixture();
  const selected = selectBuild(run, artifacts, repository);
  for (const object of [
    { type: 'commit', sha: '0'.repeat(40) },
    { type: 'tree', sha: selected.sha },
  ]) {
    const github = { rest: { git: { getRef: async () => ({ data: { object } }) } } };
    await assert.rejects(verifyReleaseTag(github, {}, selected), /原构建提交/);
  }
});

test('标签查询的权限和网络错误保留原始原因', async () => {
  const { run, artifacts } = fixture();
  const selected = selectBuild(run, artifacts, repository);
  for (const status of [403, 500]) {
    const failure = Object.assign(new Error('GitHub API failed'), { status });
    const github = { rest: { git: { getRef: async () => { throw failure; } } } };
    await assert.rejects(verifyReleaseTag(github, {}, selected), (error) => error === failure);
  }
});

test('旧全量构建仅提取 A53 必需归档与安装包', () => {
  const { run, artifacts } = fixture();
  artifacts.push({ ...artifacts[0], id: 3, name: 'honk-core-x86_64-unknown-linux-musl-full' });
  assert.equal(selectBuild(run, artifacts, repository).artifactIds, '1,2');
});

test('失败、未完成、其他分支及其他工作流不能作为发布来源', () => {
  for (const change of [
    { conclusion: 'failure' }, { status: 'in_progress' }, { head_branch: 'feature' },
    { event: 'pull_request' }, { path: '.github/workflows/ci.yml' },
    { head_repository: { full_name: 'someone/honk' } },
    { repository: { id: 200, full_name: 'someone/honk' } },
    { head_sha: 'main' }, { id: 0 },
  ]) {
    const { run, artifacts } = fixture();
    assert.throws(() => selectBuild({ ...run, ...change }, artifacts, repository));
  }
});

test('缺少 OpenWrt、重复核心产物或过期及其他提交的产物均被拒绝', () => {
  const { run, artifacts } = fixture();
  assert.throws(() => selectBuild(run, artifacts.slice(0, -1), repository), /完整/);
  assert.throws(() => selectBuild(run, [...artifacts.slice(1), artifacts[1]], repository), /完整/);
  for (const change of [
    { expired: true }, { size_in_bytes: 0 },
    { workflow_run: { ...artifacts[0].workflow_run, id: run.id + 1 } },
    { workflow_run: { ...artifacts[0].workflow_run, head_sha: '0'.repeat(40) } },
    { workflow_run: { ...artifacts[0].workflow_run, head_repository_id: 200 } },
  ]) {
    const changed = [{ ...artifacts[0], ...change }, ...artifacts.slice(1)];
    assert.throws(() => selectBuild(run, changed, repository), /过期|来源/);
  }
});
