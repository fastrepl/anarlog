import {
  BarElement,
  CategoryScale,
  Chart as ChartJS,
  LinearScale,
  Tooltip,
  type TooltipItem,
} from "chart.js";
import { Bar } from "react-chartjs-2";

ChartJS.register(CategoryScale, LinearScale, BarElement, Tooltip);

function generateRainbowColors(count: number): string[] {
  const colors: string[] = [];
  for (let i = 0; i < count; i++) {
    const hue = (i / count) * 360;
    colors.push(`hsl(${hue}, 70%, 60%)`);
  }
  return colors;
}

export function ChartSkeleton() {
  const barCount = 15;
  const heights = [95, 88, 82, 76, 70, 64, 58, 52, 46, 40, 35, 30, 26, 22, 18];

  return (
    <div className="flex h-full w-full flex-col">
      <div className="flex flex-1 items-end justify-around pb-8">
        {heights.slice(0, barCount).map((height, i) => (
          <div
            key={i}
            className="w-6 animate-pulse rounded bg-neutral-200"
            style={{ height: `${height}%` }}
          />
        ))}
      </div>
      <div className="flex h-8 justify-around">
        {Array.from({ length: barCount }).map((_, i) => (
          <div
            key={i}
            className="h-3 w-6 animate-pulse rounded bg-neutral-200"
          />
        ))}
      </div>
    </div>
  );
}

const chartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: {
      display: false,
    },
    tooltip: {
      callbacks: {
        label: (context: TooltipItem<"bar">) => {
          const value = context.parsed.y;
          return value !== null ? `${value.toFixed(1)}%` : "";
        },
      },
    },
  },
  scales: {
    x: {
      grid: {
        display: false,
      },
      ticks: {
        color: "#525252",
        maxRotation: 90,
        minRotation: 45,
        font: {
          size: 10,
        },
      },
    },
    y: {
      min: 0,
      max: 100,
      grid: {
        color: "rgba(0, 0, 0, 0.1)",
      },
      ticks: {
        color: "#525252",
        stepSize: 25,
        callback: (value: number | string) => `${value}`,
      },
      title: {
        display: true,
        text: "Success Rate (%)",
        color: "#525252",
      },
    },
  },
};

export function EvalChart({
  data,
}: {
  data: Array<{ model: string; rate: number }>;
}) {
  const sortedData = [...data].sort((a, b) => b.rate - a.rate);
  const colors = generateRainbowColors(sortedData.length);

  const chartData = {
    labels: sortedData.map((d) => d.model),
    datasets: [
      {
        data: sortedData.map((d) => d.rate),
        backgroundColor: colors,
        borderRadius: 4,
        barThickness: 24,
      },
    ],
  };

  return <Bar data={chartData} options={chartOptions} />;
}
