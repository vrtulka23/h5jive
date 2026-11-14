// Keep the direct v0.1.2 archive links usable when the GitHub API is unavailable.
const releaseAssets = document.querySelectorAll('[data-release-asset]');

if (releaseAssets.length) {
  fetch('https://api.github.com/repos/vrtulka23/h5jive/releases/latest', {
    headers: { Accept: 'application/vnd.github+json' },
  })
    .then((response) => {
      if (!response.ok) throw new Error(`GitHub release lookup failed: ${response.status}`);
      return response.json();
    })
    .then((release) => {
      if (!Array.isArray(release.assets)) return;
      const assets = new Map(release.assets.map((asset) => [asset.name, asset.browser_download_url]));
      const portable = [...releaseAssets].filter((link) => !link.classList.contains('download-installer'));
      if (!portable.every((link) => assets.has(link.dataset.releaseAsset))) return;
      for (const link of releaseAssets) {
        const url = assets.get(link.dataset.releaseAsset);
        if (!url) continue;
        link.href = url;
        link.hidden = false;
      }
      const version = document.querySelector('[data-release-version]');
      if (version && release.tag_name) version.textContent = release.tag_name;
    })
    .catch(() => {});
}
