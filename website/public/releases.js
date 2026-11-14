// Direct latest-release links stay usable when the GitHub API is unavailable.
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
      for (const link of releaseAssets) {
        const url = assets.get(link.dataset.releaseAsset);
        if (url) link.href = url;
        link.hidden = !url;
      }
      const version = document.querySelector('[data-release-version]');
      if (version && release.tag_name) version.textContent = `(${release.tag_name})`;
      const releasePage = document.querySelector('[data-release-page]');
      if (releasePage && release.html_url) releasePage.href = release.html_url;
      const checksum = document.querySelector('[data-release-checksum]');
      if (checksum) {
        const url = assets.get('SHA256SUMS');
        if (url) checksum.href = url;
        else checksum.hidden = true;
      }
    })
    .catch(() => {});
}
