# Flowtrace UI Dashboard

A modern, LinkedIn-inspired observability dashboard for Flowtrace LLM tracing.

## Features

- **Real-time Trace Monitoring**: Live view of LLM agent traces
- **Cost Tracking**: Track token usage and costs across models
- **Quality Metrics**: Monitor hallucination, relevance, and groundedness scores
- **Performance Analytics**: View latency, throughput, and error rates
- **Integration**: Connects to Langfuse, Prometheus, and Jaeger

## Quick Start

### Prerequisites

- Node.js 18+ and npm
- Docker and Docker Compose (for observability stack)

### Installation

```bash
cd ui
npm install
```

### Configuration

Create `.env.local` file:

```env
NEXT_PUBLIC_LANGFUSE_URL=http://localhost:3000
NEXT_PUBLIC_PROMETHEUS_URL=http://localhost:9091
NEXT_PUBLIC_JAEGER_URL=http://localhost:16686

# Optional: Langfuse API keys
NEXT_PUBLIC_LANGFUSE_PUBLIC_KEY=your_public_key
LANGFUSE_SECRET_KEY=your_secret_key
```

### Development

```bash
npm run dev
```

Open [http://localhost:3002](http://localhost:3002) in your browser.

### Production Build

```bash
npm run build
npm start
```

## Project Structure

```
ui/
├── app/
│   ├── dashboard/       # Main dashboard page
│   ├── layout.tsx       # Root layout
│   ├── page.tsx         # Home page
│   └── globals.css      # Global styles
├── components/
│   ├── dashboard/       # Dashboard components
│   └── ui/              # Reusable UI components
├── lib/
│   └── api/             # API clients (Langfuse, Prometheus)
├── public/              # Static assets
└── package.json
```

## API Integration

### Langfuse

The dashboard fetches trace data from Langfuse:

- Traces: `GET /api/public/traces`
- Trace details: `GET /api/public/traces/:id`
- Metrics: `GET /api/public/metrics`

### Prometheus

Queries system and LLM metrics:

- Request rate: `rate(flowtrace_llm_requests_total[5m])`
- Latency: `histogram_quantile(0.95, rate(flowtrace_llm_latency_seconds_bucket[5m]))`
- Costs: `increase(flowtrace_llm_cost_total[1h])`

## Design System

Based on LinkedIn's professional design language:

- **Primary Color**: #0A66C2 (LinkedIn Blue)
- **Typography**: System font stack
- **Components**: Cards, buttons, tables with consistent spacing
- **Responsive**: Mobile-first design with breakpoints

## Customization

### Colors

Edit `tailwind.config.js` to customize the color palette.

### Layout

Modify `app/dashboard/page.tsx` to add/remove dashboard sections.

### Metrics

Add custom Prometheus queries in `lib/api/prometheus.ts`.

## Troubleshooting

**Issue: Cannot connect to Langfuse**
- Ensure the observability stack is running: `cd observability && docker-compose up -d`
- Check Langfuse is accessible at http://localhost:3000

**Issue: No data showing**
- Verify that Flowtrace is sending traces to the OTLP collector
- Check browser console for API errors
- Enable mock data mode for testing (enabled by default)

## License

Apache 2.0
