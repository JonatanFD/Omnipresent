import { Carousel, CarouselContent, CarouselItem } from "../ui/carousel";
import { WelcomeSlide1 } from "./slide-1";
import { WelcomeSlide2 } from "./slide-2";
import { WelcomeSlide3 } from "./slide-3";

const welcomeWrapperItems = [
  {
    title: "Intro",
    content: <WelcomeSlide1 />,
  },
  {
    title: "Control your PC",
    content: <WelcomeSlide2 />,
  },
  {
    title: "Multiplatform",
    content: <WelcomeSlide3 />,
  },
];

export function WelcomeView() {
  return (
    <section className="w-full relative">
      <Carousel opts={{ watchDrag: false }}>
        <CarouselContent>
          {welcomeWrapperItems.map((item) => (
            <CarouselItem key={item.title}>
              <main>{item.content}</main>
            </CarouselItem>
          ))}
        </CarouselContent>
      </Carousel>
    </section>
  );
}
