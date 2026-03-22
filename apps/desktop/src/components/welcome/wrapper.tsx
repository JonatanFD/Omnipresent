import { Carousel, CarouselContent, CarouselItem } from "../ui/carousel";
import { WelcomeSlide1 } from "./slide-1";
import { WelcomeSlide2 } from "./slide-2";
import { WelcomeSlide3 } from "./slide-3";

const slides = [
  { title: "Intro", content: <WelcomeSlide1 /> },
  { title: "Control your PC", content: <WelcomeSlide2 /> },
  { title: "Multiplatform", content: <WelcomeSlide3 /> },
];

export function WelcomeView() {
  return (
    <section className="w-full">
      <Carousel opts={{ watchDrag: false }}>
        <CarouselContent>
          {slides.map((slide) => (
            <CarouselItem key={slide.title}>{slide.content}</CarouselItem>
          ))}
        </CarouselContent>
      </Carousel>
    </section>
  );
}
