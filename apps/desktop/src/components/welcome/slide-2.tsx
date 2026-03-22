import { ArrowLeft, ArrowRight } from "lucide-react";
import { useCarousel } from "../ui/carousel";
import { Button } from "../ui/button";
import { Android } from "../ui/android";
import { Safari } from "../ui/safari";

export function WelcomeSlide2() {
  const { scrollNext, scrollPrev } = useCarousel();

  return (
    <article className="min-h-screen flex flex-col items-center justify-between px-8 py-16 bg-background relative overflow-hidden">
      {/* Radial glow */}
      <div
        className="pointer-events-none absolute inset-0 opacity-15"
        style={{
          background:
            "radial-gradient(ellipse 70% 40% at 50% 50%, hsl(var(--primary) / 0.4), transparent)",
        }}
      />

      <StepDots current={1} total={3} />

      {/* Illustration */}
      <div className="flex-1 flex items-center justify-center w-full">
        <div className="relative flex items-center justify-center gap-6 animate-slide-up">
          {/* Phone */}
          <div className="relative z-10 drop-shadow-xl">
            <Android className="w-28 h-auto" />
          </div>

          {/* Connection line */}
          <div className="flex items-center gap-1">
            {[0, 1, 2, 3].map((i) => (
              <span
                key={i}
                className="block w-1.5 h-1.5 rounded-full bg-primary/40"
                style={{ animationDelay: `${i * 0.15}s` }}
              />
            ))}
          </div>

          {/* Desktop */}
          <div className="relative z-10 w-64 drop-shadow-xl">
            <Safari mode="simple" />
          </div>
        </div>
      </div>

      {/* Text + nav */}
      <div className="w-full max-w-sm space-y-6">
        <div className="text-center space-y-2">
          <h2 className="text-2xl font-semibold tracking-tight">
            Remote control
          </h2>
          <p className="text-muted-foreground text-sm leading-relaxed">
            Use your smartphone as a controller. Your PC responds in real time
            over your local network.
          </p>
        </div>

        <div className="flex items-center justify-between">
          <Button
            variant="ghost"
            size="icon"
            onClick={scrollPrev}
            className="rounded-full h-10 w-10"
          >
            <ArrowLeft className="w-4 h-4" />
          </Button>
          <Button
            size="default"
            onClick={scrollNext}
            className="gap-2 px-6 rounded-full"
          >
            Next
            <ArrowRight className="w-4 h-4" />
          </Button>
        </div>
      </div>
    </article>
  );
}

function StepDots({ current, total }: { current: number; total: number }) {
  return (
    <div className="flex items-center gap-1.5">
      {Array.from({ length: total }).map((_, i) => (
        <span
          key={i}
          className={`block rounded-full transition-all duration-300 ${
            i === current
              ? "w-5 h-1.5 bg-foreground"
              : "w-1.5 h-1.5 bg-muted-foreground/30"
          }`}
        />
      ))}
    </div>
  );
}
