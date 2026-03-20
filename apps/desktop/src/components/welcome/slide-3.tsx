import { ArrowLeft, ArrowRight } from "lucide-react";
import { useCarousel } from "../ui/carousel";
import { Button } from "../ui/button";
import { Linux } from "../ui/svgs/linux";
import MacOS from "../ui/svgs/macos";
import Windows from "../ui/svgs/windows";

export function WelcomeSlide3() {
  const { scrollNext, scrollPrev } = useCarousel();
  return (
    <article className="min-h-screen flex flex-col">
      <div className="bg-muted h-120 flex justify-center items-center flex-col gap-10">
        <ul className="flex items-center gap-20">
          <li>
            <Windows className="size-20" />
          </li>
          <li>
            <MacOS className="size-20" />
          </li>
          <li>
            <Linux className="size-20" />
          </li>
        </ul>
        <h1 className="text-4xl font-bold">Multiplatform</h1>
      </div>
      <div className="flex-1 space-x-4 flex p-4">
        <div className="flex items-center">
          <Button onClick={scrollPrev} size="icon-xl">
            <ArrowLeft />
          </Button>
        </div>

        <section className="flex-1 px-4 h-full items-center">
          <p>Omnipresent works fine on all platforms.</p>
        </section>

        <div className="flex items-center">
          <Button onClick={scrollNext} size="icon-xl">
            <ArrowRight />
          </Button>
        </div>
      </div>
    </article>
  );
}
