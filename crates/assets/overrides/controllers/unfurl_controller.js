// The reference's unfurl_controller.js, extended for inline image links (README, "Known
// differences"): an unfurl that answers an image with no title and no URL is a URL that points at
// an image (or a GIPHY page's resolved media URL), and the pasted link is replaced with it as a
// remote image attachment.
import { Controller } from "@hotwired/stimulus"
import { post } from "@rails/request.js"
import { escapeHTML, truncateString } from "helpers/string_helpers"

const OPENGRAPH_EMBED_CONTENT_TYPE = "application/vnd.actiontext.opengraph-embed"
const INLINE_IMAGE_CONTENT_TYPE = "image/*"

const UNFURLED_TWITTER_AVATAR_CSS_CLASS = "og-embed--twitter-avatar"
const TWITTER_AVATAR_URL_PREFIX = "https://pbs.twimg.com/profile_images"

export default class extends Controller {
  #abortController

  disconnect() {
    this.#abortController?.abort()
  }

  async unfurl(event) {
    if (!this.element.permitsAttachmentContentType(OPENGRAPH_EMBED_CONTENT_TYPE) && !this.element.permitsAttachmentContentType(INLINE_IMAGE_CONTENT_TYPE)) return

    const { url, insertBelowLink, replaceLinkWith } = event.detail

    const metadata = await this.#fetchOpengraphMetadata(url)
    if (!metadata) return

    // An image-only answer has no title and no link URL to show: the image and the media content
    // type stand in for a plain link instead.
    if (metadata.image && !metadata.title) {
      if (this.element.permitsAttachmentContentType(INLINE_IMAGE_CONTENT_TYPE)) {
        replaceLinkWith(this.#inlineImageHTML(metadata.image))
        return
      }
    }

    if (metadata.title && metadata.href && this.element.permitsAttachmentContentType(OPENGRAPH_EMBED_CONTENT_TYPE)) {
      insertBelowLink(this.#opengraphEmbedHTML(metadata), { attachment: { contentType: OPENGRAPH_EMBED_CONTENT_TYPE } })
    }
  }

  async #fetchOpengraphMetadata(url) {
    this.#abortController?.abort()
    this.#abortController = new AbortController()

    try {
      const response = await post("/unfurl_link", {
        body: { url },
        contentType: "application/json",
        signal: this.#abortController.signal
      })

      if (response.ok && response.statusCode !== 204) {
        const { title, url: href, image, description } = await response.json
        if (title && href) return { title, href, image, description }
        if (image) return { image }
      }
    } catch {
      // Ignore aborted and failed requests
    }

    return null
  }

  #inlineImageHTML(url) {
    return `<img src="${escapeHTML(url)}" alt="" />`
  }

  #opengraphEmbedHTML({ title, href, image, description }) {
    return `<actiontext-opengraph-embed>
      <div class="og-embed gap ${this.#embedClass(image)}">
        <div class="og-embed__content">
          <div class="og-embed__title">
            <a href="${escapeHTML(href)}" rel="noreferrer" target="_blank">${escapeHTML(truncateString(title, 280))}</a>
          </div>
          <div class="og-embed__description">${escapeHTML(truncateString(description, 560))}</div>
        </div>
        ${this.#imageHTML(image)}
      </div>
    </actiontext-opengraph-embed>`
  }

  #embedClass(image) {
    if (this.#isTwitterAvatar(image)) {
      return UNFURLED_TWITTER_AVATAR_CSS_CLASS
    } else {
      return ""
    }
  }

  #isTwitterAvatar(image) {
    return Boolean(image) && image.startsWith(TWITTER_AVATAR_URL_PREFIX)
  }

  #imageHTML(image) {
    if (image) {
      return `<div class="og-embed__image"><img src="${escapeHTML(image)}" class="image center" alt="" /></div>`
    } else {
      return ""
    }
  }
}
