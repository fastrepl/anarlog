import { ArrowLeft, ArrowRight } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import useEmblaCarousel, {
  type UseEmblaCarouselType,
} from "embla-carousel-react";
import * as React from "react";

import { radii } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

import { Button } from "./button";

type CarouselApi = UseEmblaCarouselType[1];
type UseCarouselParameters = Parameters<typeof useEmblaCarousel>;
type CarouselOptions = UseCarouselParameters[0];
type CarouselPlugin = UseCarouselParameters[1];

type CarouselProps = {
  opts?: CarouselOptions;
  plugins?: CarouselPlugin;
  orientation?: "horizontal" | "vertical";
  setApi?: (api: CarouselApi) => void;
};

type CarouselContextProps = {
  carouselRef: ReturnType<typeof useEmblaCarousel>[0];
  api: ReturnType<typeof useEmblaCarousel>[1];
  scrollPrev: () => void;
  scrollNext: () => void;
  canScrollPrev: boolean;
  canScrollNext: boolean;
} & CarouselProps;

const CarouselContext = React.createContext<CarouselContextProps | null>(null);

function useCarousel() {
  const context = React.useContext(CarouselContext);

  if (!context) {
    throw new Error("useCarousel must be used within a <Carousel />");
  }

  return context;
}

const Carousel = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement> & CarouselProps & StyleXProps
>(
  (
    {
      orientation = "horizontal",
      opts,
      setApi,
      plugins,
      className,
      style,
      sx,
      children,
      ...props
    },
    ref,
  ) => {
    const [carouselRef, api] = useEmblaCarousel(
      {
        ...opts,
        axis: orientation === "horizontal" ? "x" : "y",
        duration: 20,
      },
      plugins,
    );
    const [canScrollPrev, setCanScrollPrev] = React.useState(false);
    const [canScrollNext, setCanScrollNext] = React.useState(false);

    const onSelect = React.useCallback((api: CarouselApi) => {
      if (!api) {
        return;
      }

      setCanScrollPrev(api.canScrollPrev());
      setCanScrollNext(api.canScrollNext());
    }, []);

    const scrollPrev = React.useCallback(() => {
      api?.scrollPrev();
    }, [api]);

    const scrollNext = React.useCallback(() => {
      api?.scrollNext();
    }, [api]);

    const handleKeyDown = React.useCallback(
      (event: React.KeyboardEvent<HTMLDivElement>) => {
        if (event.key === "ArrowLeft") {
          event.preventDefault();
          scrollPrev();
        } else if (event.key === "ArrowRight") {
          event.preventDefault();
          scrollNext();
        }
      },
      [scrollPrev, scrollNext],
    );

    React.useEffect(() => {
      if (!api || !setApi) {
        return;
      }

      setApi(api);
    }, [api, setApi]);

    React.useEffect(() => {
      if (!api) {
        return;
      }

      onSelect(api);
      api.on("reInit", onSelect);
      api.on("select", onSelect);

      return () => {
        api?.off("select", onSelect);
      };
    }, [api, onSelect]);

    return (
      <CarouselContext.Provider
        value={{
          carouselRef,
          api: api,
          opts,
          orientation:
            orientation || (opts?.axis === "y" ? "vertical" : "horizontal"),
          scrollPrev,
          scrollNext,
          canScrollPrev,
          canScrollNext,
        }}
      >
        <div
          ref={ref}
          onKeyDownCapture={handleKeyDown}
          role="region"
          aria-roledescription="carousel"
          {...props}
          {...mergeStyleXProps([styles.root, sx], className, style)}
        >
          {children}
        </div>
      </CarouselContext.Provider>
    );
  },
);
Carousel.displayName = "Carousel";

const CarouselContent = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement> & StyleXProps
>(({ className, style, sx, ...props }, ref) => {
  const { carouselRef, orientation } = useCarousel();

  return (
    <div ref={carouselRef} {...mergeStyleXProps(styles.viewport)}>
      <div
        ref={ref}
        {...props}
        {...mergeStyleXProps(
          [
            styles.content,
            carouselContentOrientationStyles[orientation ?? "horizontal"],
            sx,
          ],
          className,
          style,
        )}
      />
    </div>
  );
});
CarouselContent.displayName = "CarouselContent";

const CarouselItem = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement> & StyleXProps
>(({ className, style, sx, ...props }, ref) => {
  const { orientation } = useCarousel();

  return (
    <div
      ref={ref}
      role="group"
      aria-roledescription="slide"
      {...props}
      {...mergeStyleXProps(
        [
          styles.item,
          carouselItemOrientationStyles[orientation ?? "horizontal"],
          sx,
        ],
        className,
        style,
      )}
    />
  );
});
CarouselItem.displayName = "CarouselItem";

const CarouselPrevious = React.forwardRef<
  HTMLButtonElement,
  React.ComponentProps<typeof Button>
>(
  (
    { className, style, sx, variant = "outline", size = "icon", ...props },
    ref,
  ) => {
    const { orientation, scrollPrev, canScrollPrev } = useCarousel();
    const iconStyle = mergeStyleXProps(styles.navigationIcon);

    if (!canScrollPrev) {
      return null;
    }

    return (
      <Button
        ref={ref}
        variant={variant}
        size={size}
        disabled={!canScrollPrev}
        onClick={scrollPrev}
        {...props}
        className={className}
        style={style}
        sx={[
          styles.navigationButton,
          carouselPreviousOrientationStyles[orientation ?? "horizontal"],
          sx,
        ]}
      >
        <ArrowLeft className={iconStyle.className} style={iconStyle.style} />
        <span {...stylex.props(styles.visuallyHidden)}>Previous slide</span>
      </Button>
    );
  },
);
CarouselPrevious.displayName = "CarouselPrevious";

const CarouselNext = React.forwardRef<
  HTMLButtonElement,
  React.ComponentProps<typeof Button>
>(
  (
    { className, style, sx, variant = "outline", size = "icon", ...props },
    ref,
  ) => {
    const { orientation, scrollNext, canScrollNext } = useCarousel();
    const iconStyle = mergeStyleXProps(styles.navigationIcon);

    if (!canScrollNext) {
      return null;
    }

    return (
      <Button
        ref={ref}
        variant={variant}
        size={size}
        disabled={!canScrollNext}
        onClick={scrollNext}
        {...props}
        className={className}
        style={style}
        sx={[
          styles.navigationButton,
          carouselNextOrientationStyles[orientation ?? "horizontal"],
          sx,
        ]}
      >
        <ArrowRight className={iconStyle.className} style={iconStyle.style} />
        <span {...stylex.props(styles.visuallyHidden)}>Next slide</span>
      </Button>
    );
  },
);
CarouselNext.displayName = "CarouselNext";

const styles = stylex.create({
  root: {
    position: "relative",
  },
  viewport: {
    overflow: "hidden",
  },
  content: {
    display: "flex",
  },
  contentHorizontal: {
    marginLeft: "-1rem",
  },
  contentVertical: {
    flexDirection: "column",
    marginTop: "-1rem",
  },
  item: {
    flexBasis: "100%",
    flexGrow: 0,
    flexShrink: 0,
    minWidth: 0,
  },
  itemHorizontal: {
    paddingLeft: "1rem",
  },
  itemVertical: {
    paddingTop: "1rem",
  },
  navigationButton: {
    borderRadius: radii.full,
    height: "2rem",
    position: "absolute",
    width: "2rem",
  },
  previousHorizontal: {
    left: "-3rem",
    top: "50%",
    transform: "translateY(-50%)",
  },
  previousVertical: {
    left: "50%",
    top: "-3rem",
    transform: "translateX(-50%) rotate(90deg)",
  },
  nextHorizontal: {
    right: "-3rem",
    top: "50%",
    transform: "translateY(-50%)",
  },
  nextVertical: {
    bottom: "-3rem",
    left: "50%",
    transform: "translateX(-50%) rotate(90deg)",
  },
  navigationIcon: {
    height: "1rem",
    width: "1rem",
  },
  visuallyHidden: {
    borderWidth: 0,
    clip: "rect(0, 0, 0, 0)",
    height: "1px",
    margin: "-1px",
    overflow: "hidden",
    padding: 0,
    position: "absolute",
    whiteSpace: "nowrap",
    width: "1px",
  },
});

const carouselContentOrientationStyles = {
  horizontal: styles.contentHorizontal,
  vertical: styles.contentVertical,
};

const carouselItemOrientationStyles = {
  horizontal: styles.itemHorizontal,
  vertical: styles.itemVertical,
};

const carouselPreviousOrientationStyles = {
  horizontal: styles.previousHorizontal,
  vertical: styles.previousVertical,
};

const carouselNextOrientationStyles = {
  horizontal: styles.nextHorizontal,
  vertical: styles.nextVertical,
};

export {
  Carousel,
  type CarouselApi,
  CarouselContent,
  CarouselItem,
  CarouselNext,
  CarouselPrevious,
};
