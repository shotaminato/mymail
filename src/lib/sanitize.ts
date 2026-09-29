const DANGEROUS_TAGS = new Set([
  "script",
  "iframe",
  "object",
  "embed",
  "link",
  "meta",
  "base",
  "form",
  "input",
  "button",
  "textarea",
  "select",
  "svg",
  "math",
  "frame",
  "frameset",
]);

function csp(allowRemoteImages: boolean): string {
  const img = allowRemoteImages ? "img-src https: http: data:;" : "img-src data:;";
  return `default-src 'none'; ${img} style-src 'unsafe-inline'; font-src data:;`;
}

function stripDanger(doc: Document, allowRemoteImages: boolean) {
  doc.querySelectorAll("*").forEach((el) => {
    const tag = el.tagName.toLowerCase();
    if (DANGEROUS_TAGS.has(tag)) {
      el.remove();
      return;
    }
    for (const attr of [...el.attributes]) {
      const name = attr.name.toLowerCase();
      const value = attr.value.trim();
      if (name.startsWith("on") || name === "srcdoc" || name === "xlink:href") {
        el.removeAttribute(attr.name);
        continue;
      }
      if (
        (name === "href" || name === "src" || name === "action" || name === "formaction") &&
        /^(javascript|vbscript|data):/i.test(value)
      ) {
        el.removeAttribute(attr.name);
      }
    }
    if (tag === "a") {
      el.setAttribute("target", "_blank");
      el.setAttribute("rel", "noopener noreferrer");
    }
    if (tag === "img" && !allowRemoteImages) {
      const src = el.getAttribute("src") ?? "";
      if (/^https?:/i.test(src)) {
        el.setAttribute("alt", el.getAttribute("alt") || "blocked remote image");
        el.removeAttribute("src");
        el.setAttribute("data-blocked-src", src);
      }
    }
  });
}

export function htmlToSrcdoc(html: string, allowRemoteImages: boolean): string {
  const parsed = new DOMParser().parseFromString(html, "text/html");
  stripDanger(parsed, allowRemoteImages);
  const body = parsed.body.innerHTML;
  return `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${csp(allowRemoteImages)}">
<style>
  html, body {
    margin: 0;
    padding: 12px 16px;
    font: 14px/1.5 system-ui, sans-serif;
    color: #1c1917;
    background: #fff;
    overflow-wrap: anywhere;
  }
  img { max-width: 100%; height: auto; }
  a { color: #1d4ed8; }
</style>
</head>
<body>${body}</body>
</html>`;
}
