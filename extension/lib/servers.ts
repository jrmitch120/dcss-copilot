// Where webtiles games are played. The Community Statistics repo will own the public server list;
// until then it's kept here. Match patterns ignore ports, so each covers every port on its host.

/** Public webtiles servers, plus local webtiles. */
export const WEBTILES_MATCHES = [
  "*://crawl.akrasiac.org/*",
  "*://cbro.berotato.org/*",
  "*://crawl.develz.org/*",
  "*://crawl.dcss.io/*",
  "*://crawl.nemelex.cards/*",
  "*://crawl.project357.org/*",
  "*://crawl.xtahua.com/*",
  "*://underhound.eu/*",
  "http://localhost/*",
  "http://127.0.0.1/*",
];
