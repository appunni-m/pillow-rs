// FontFace loader for pillow-rs WASM — works in browser and Node.js
// Browser: const font = await loadFont('https://fonts.example.com/roboto.ttf', 16);
// Server:  const font = loadFontSync('/usr/share/fonts/arial.ttf', 16);

import { ImageFont } from './pkg/pillow_rs_js.js';

/**
 * Load a font from URL (browser) or file path (Node.js server).
 * Returns an ImageFont instance ready for use with ImageDraw.text().
 * 
 * @param {string|Uint8Array} source - URL string (browser), file path (server), or raw bytes
 * @param {number} size - Font size in points
 * @returns {Promise<ImageFont>} Font instance
 */
export async function loadFont(source, size = 16) {
    let data;
    
    if (source instanceof Uint8Array) {
        data = source;
    } else if (typeof source === 'string') {
        // Browser: use FontFace API
        if (typeof window !== 'undefined' && typeof FontFace !== 'undefined') {
            const fontFace = new FontFace('pillow-rs-font', `url(${source})`);
            await fontFace.load();
            // FontFace loaded — now fetch the raw bytes
            const response = await fetch(source);
            data = new Uint8Array(await response.arrayBuffer());
        }
        // Node.js server: read from filesystem
        else if (typeof require !== 'undefined') {
            const fs = require('fs');
            data = new Uint8Array(fs.readFileSync(source));
        }
    }
    
    if (!data) throw new Error('No font data loaded');
    return new ImageFont(data, size);
}

/**
 * Synchronous font loader for Node.js server.
 * @param {string} path - File path to .ttf font
 * @param {number} size - Font size in points
 * @returns {ImageFont}
 */
export function loadFontSync(path, size = 16) {
    const fs = require('fs');
    const data = new Uint8Array(fs.readFileSync(path));
    return new ImageFont(data, size);
}

/**
 * Load the default system font (server only).
 * Looks in common font directories for a suitable TTF.
 * @param {number} size - Font size in points  
 * @returns {ImageFont|null}
 */
export function loadDefaultFont(size = 14) {
    if (typeof require === 'undefined') return null;
    const fs = require('fs');
    const paths = [
        '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',
        '/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf',
        '/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf',
        '/System/Library/Fonts/Helvetica.ttc',
        'C:\\Windows\\Fonts\\arial.ttf',
    ];
    for (const p of paths) {
        try {
            if (fs.existsSync(p)) {
                return new ImageFont(new Uint8Array(fs.readFileSync(p)), size);
            }
        } catch(e) {}
    }
    return null;
}
