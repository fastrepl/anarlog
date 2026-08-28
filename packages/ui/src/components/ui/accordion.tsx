import { CaretDown } from "@phosphor-icons/react";
import * as AccordionPrimitive from "@radix-ui/react-accordion";
import * as stylex from "@stylexjs/stylex";
import * as React from "react";

import { colors } from "@anlg/design-system/tokens.stylex";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

const Accordion = AccordionPrimitive.Root;
const AccordionHeader = AccordionPrimitive.Header;

const AccordionItem = React.forwardRef<
  React.ComponentRef<typeof AccordionPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Item> & StyleXProps
>(({ className, style, sx, ...props }, ref) => {
  const resolvedStyle = mergeStyleXProps([styles.item, sx], className, style);

  return (
    <AccordionPrimitive.Item
      {...props}
      ref={ref}
      className={resolvedStyle.className}
      style={resolvedStyle.style}
    />
  );
});
AccordionItem.displayName = "AccordionItem";

const AccordionTrigger = React.forwardRef<
  React.ComponentRef<typeof AccordionPrimitive.Trigger>,
  React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Trigger> &
    StyleXProps
>(({ className, style, sx, children, ...props }, ref) => {
  const headerStyle = mergeStyleXProps(styles.header);
  const triggerStyle = mergeStyleXProps([styles.trigger, sx], className, style);
  const iconStyle = mergeStyleXProps(styles.triggerIcon);

  return (
    <AccordionPrimitive.Header
      className={headerStyle.className}
      style={headerStyle.style}
    >
      <AccordionPrimitive.Trigger
        {...props}
        ref={ref}
        className={triggerStyle.className}
        style={triggerStyle.style}
      >
        {children}
        <CaretDown className={iconStyle.className} style={iconStyle.style} />
      </AccordionPrimitive.Trigger>
    </AccordionPrimitive.Header>
  );
});
AccordionTrigger.displayName = AccordionPrimitive.Trigger.displayName;

const AccordionContent = React.forwardRef<
  React.ComponentRef<typeof AccordionPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Content> &
    StyleXProps
>(({ className, style, sx, children, ...props }, ref) => {
  const contentStyle = mergeStyleXProps(styles.content, undefined, style);
  const innerStyle = mergeStyleXProps([styles.contentInner, sx], className);

  return (
    <AccordionPrimitive.Content
      {...props}
      ref={ref}
      className={contentStyle.className}
      style={contentStyle.style}
    >
      <div className={innerStyle.className} style={innerStyle.style}>
        {children}
      </div>
    </AccordionPrimitive.Content>
  );
});
AccordionContent.displayName = AccordionPrimitive.Content.displayName;

const AccordionTriggerPrimitive = AccordionPrimitive.Trigger;

const accordionDown = stylex.keyframes({
  from: { height: 0 },
  to: { height: "var(--radix-accordion-content-height)" },
});

const accordionUp = stylex.keyframes({
  from: { height: "var(--radix-accordion-content-height)" },
  to: { height: 0 },
});

const styles = stylex.create({
  item: {
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
  },
  header: {
    display: "flex",
  },
  trigger: {
    alignItems: "center",
    display: "flex",
    flexBasis: "0%",
    flexGrow: 1,
    flexShrink: 1,
    fontSize: "0.875rem",
    fontWeight: 500,
    justifyContent: "space-between",
    lineHeight: "1.25rem",
    paddingBlock: "1rem",
    textAlign: "left",
    textDecorationLine: {
      default: "none",
      ":hover": "underline",
    },
    transform: {
      default: null,
      ':is([data-state="open"]) > svg': "rotate(180deg)",
    },
    transitionDuration: "150ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  triggerIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    transitionDuration: "200ms",
    transitionProperty: "transform",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1rem",
  },
  content: {
    animationDuration: "200ms",
    animationName: {
      default: null,
      ':is([data-state="closed"])': accordionUp,
      ':is([data-state="open"])': accordionDown,
    },
    animationTimingFunction: "ease-out",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    overflow: "hidden",
  },
  contentInner: {
    paddingBottom: "1rem",
    paddingTop: 0,
  },
});

export {
  Accordion,
  AccordionContent,
  AccordionHeader,
  AccordionItem,
  AccordionTrigger,
  AccordionTriggerPrimitive,
};
