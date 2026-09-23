const createNextIntlPlugin = require('next-intl/plugin');
const withNextIntl = createNextIntlPlugin('./src/i18n.ts');

function imagePattern(raw, fallback) {
  let parsed;
  try {
    parsed = new URL(raw);
  } catch {
    parsed = new URL(fallback);
  }
  return {
    protocol: parsed.protocol.replace(':', ''),
    hostname: parsed.hostname,
    // Explicit ports only; protocol defaults keep local docker setups working.
    port: parsed.port || (parsed.protocol === 'https:' ? '443' : '80'),
    pathname: '/autocrm/**',
  };
}

// Overridable per environment; see frontend/.env.example.
const apiUrl = process.env.AUTOCRM_API_URL ?? 'http://localhost:8080';
const imagePatterns = process.env.AUTOCRM_S3_URL
  ? [imagePattern(process.env.AUTOCRM_S3_URL, 'http://localhost:9000')]
  : [
      imagePattern('http://localhost:9000', 'http://localhost:9000'),
      imagePattern('http://127.0.0.1:9000', 'http://127.0.0.1:9000'),
    ];

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  experimental: {
    // Barrel files: import only the icons/helpers actually used, not the whole index.
    optimizePackageImports: [
      'lucide-react',
      'date-fns',
      'recharts',
      // Not @tanstack/*: their ESM-only builds fail to parse under the optimizer.
    ],
  },
  images: {
    remotePatterns: imagePatterns,
  },
  async rewrites() {
    return [
      {
        source: '/api/:path*',
        destination: `${apiUrl}/api/:path*`,
      },
    ];
  },
};

module.exports = withNextIntl(nextConfig);
