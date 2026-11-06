// Letters point at their header logo by Content-ID (`cid:autotherm-logo`): the mail
// carries it as an inline part. A browser has no such part, so previews of a letter load
// the same file from the web app instead (public/email/autotherm-logo.png).

export function previewHtml(html: string): string {
  return html.replaceAll('cid:autotherm-logo', '/email/autotherm-logo.png');
}
