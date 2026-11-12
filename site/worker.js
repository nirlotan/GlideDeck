// nirlotan.com/GlideDeck: the GlideDeck site.
//
// The pages are static files in ./public, served by Cloudflare before this script runs.
// The script sees only:
//   - video files (see run_worker_first in wrangler.jsonc), so it can answer byte-range
//     requests. Safari will not play a video from a server that ignores them.
//   - requests that matched no file: other spellings of the address (/glidedeck,
//     /Glidedeck/...) go to /GlideDeck/, and the rest get a 404.
export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (url.pathname.endsWith('.mp4')) return video(request, env);

    const other = url.pathname.match(/^\/glidedeck(\/.*)?$/i);
    if (other && !url.pathname.startsWith('/GlideDeck')) {
      url.pathname = '/GlideDeck' + (other[1] || '/');
      return Response.redirect(url.toString(), 301);
    }
    const asset = await env.ASSETS.fetch(request);
    if (asset.status !== 404) return asset;
    return new Response(
      '<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Not found · GlideDeck</title>' +
        '<body style="margin:0;min-height:100vh;display:grid;place-content:center;gap:16px;background:#ffd23f;color:#14110a;font:600 20px system-ui,sans-serif;text-align:center">' +
        '<p style="margin:0;font-size:64px;font-weight:800">Nothing here.</p><p style="margin:0"><a href="/GlideDeck/" style="color:inherit">Fly back to GlideDeck</a></p>',
      { status: 404, headers: { 'content-type': 'text/html; charset=utf-8' } },
    );
  },
};

// Serves a video file whole, or the slice a Range header asks for.
async function video(request, env) {
  const asset = await env.ASSETS.fetch(new Request(request.url, { method: 'GET' }));
  if (!asset.ok) return asset;
  const headers = new Headers(asset.headers);
  headers.set('accept-ranges', 'bytes');
  headers.set('cache-control', 'public, max-age=86400');
  const range = /^bytes=(\d*)-(\d*)$/.exec(request.headers.get('range') || '');
  if (!range || (range[1] === '' && range[2] === '')) {
    return new Response(request.method === 'HEAD' ? null : asset.body, { status: 200, headers });
  }
  const bytes = await asset.arrayBuffer();
  const size = bytes.byteLength;
  let start, end;
  if (range[1] === '') {
    // "bytes=-500": the last 500 bytes.
    start = Math.max(0, size - Number(range[2]));
    end = size - 1;
  } else {
    start = Number(range[1]);
    end = range[2] === '' ? size - 1 : Math.min(Number(range[2]), size - 1);
  }
  if (start >= size || start > end) {
    return new Response(null, { status: 416, headers: { 'content-range': `bytes */${size}` } });
  }
  headers.set('content-range', `bytes ${start}-${end}/${size}`);
  headers.set('content-length', String(end - start + 1));
  return new Response(request.method === 'HEAD' ? null : bytes.slice(start, end + 1), { status: 206, headers });
}
