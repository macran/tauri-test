// // stepper.tsx
// import React from "react";
// import { cn } from "@/utils";

// type Orientation = "vertical" | "horizontal";
// type StepState = "inactive" | "active" | "completed";

// type StepperContextValue = {
//   activeStep: number;
//   maxStep: number;
//   orientation: Orientation;
//   canGoToStep: (step: number) => boolean;
//   goToStep: (step: number) => void;
//   next: () => Promise<void>;
//   back: () => void;
//   isTransitioning: boolean;
// };

// const StepperContext = React.createContext<StepperContextValue | null>(null);
// export function useStepper() {
//   const ctx = React.useContext(StepperContext);
//   if (!ctx) throw new Error("useStepper must be used within <StepperRoot />");
//   return ctx;
// }

// type StepperRootProps = React.HTMLAttributes<HTMLDivElement> & {
//   steps: number;
//   activeStep: number; // 1-based
//   onActiveStepChange: (step: number) => void;
//   orientation?: Orientation;
//   beforeNext?: (currentStep: number) => boolean | Promise<boolean>;
//   forbidFutureClick?: boolean;
// };
// export function StepperRoot({
//   steps,
//   activeStep,
//   onActiveStepChange,
//   orientation = "vertical",
//   beforeNext,
//   forbidFutureClick = true,
//   className,
//   ...props
// }: StepperRootProps) {
//   const [isTransitioning, setIsTransitioning] = React.useState(false);

//   const canGoToStep = React.useCallback(
//     (step: number) => {
//       if (step < 1 || step > steps) return false;
//       if (!forbidFutureClick) return true;
//       return step < activeStep; // 仅允许回退
//     },
//     [steps, forbidFutureClick, activeStep],
//   );

//   const goToStep = React.useCallback(
//     (step: number) => {
//       if (!canGoToStep(step)) return;
//       onActiveStepChange(step);
//     },
//     [canGoToStep, onActiveStepChange],
//   );

//   const back = React.useCallback(
//     () => onActiveStepChange(Math.max(1, activeStep - 1)),
//     [activeStep, onActiveStepChange],
//   );

//   const next = React.useCallback(async () => {
//     if (activeStep >= steps) return;
//     if (isTransitioning) return;
//     try {
//       setIsTransitioning(true);
//       const ok = beforeNext ? await beforeNext(activeStep) : true;
//       if (ok) onActiveStepChange(activeStep + 1);
//     } finally {
//       setIsTransitioning(false);
//     }
//   }, [activeStep, steps, beforeNext, onActiveStepChange, isTransitioning]);

//   const ctx: StepperContextValue = {
//     activeStep,
//     maxStep: steps,
//     orientation,
//     canGoToStep,
//     goToStep,
//     next,
//     back,
//     isTransitioning,
//   };

//   return (
//     <StepperContext.Provider value={ctx}>
//       <div
//         data-orientation={orientation}
//         className={cn(
//           orientation === "vertical"
//             ? "flex w-full flex-col justify-start gap-10"
//             : "flex w-full flex-row items-start gap-6",
//           className,
//         )}
//         {...props}
//       />
//     </StepperContext.Provider>
//   );
// }

// type StepItemRenderProps = {
//   state: StepState;
//   step: number;
//   isClickable: boolean;
// };
// type StepperItemProps = Omit<
//   React.HTMLAttributes<HTMLDivElement>,
//   "children"
// > & {
//   step: number;
//   children: React.ReactNode | ((args: StepItemRenderProps) => React.ReactNode);
// };

// const StepperItemContext = React.createContext<StepItemRenderProps | null>(
//   null,
// );
// function useStepItem() {
//   const ctx = React.useContext(StepperItemContext);
//   if (!ctx) throw new Error("StepperItem must be used within a StepperItem");
//   return ctx;
// }

// export function StepperItem({
//   step,
//   className,
//   children,
//   ...props
// }: StepperItemProps) {
//   const { activeStep } = useStepper();
//   const state: StepState =
//     step < activeStep
//       ? "completed"
//       : step === activeStep
//         ? "active"
//         : "inactive";
//   const isClickable = step < activeStep;
//   return (
//     <StepperItemContext.Provider value={{ state, step, isClickable }}>
//       <div
//         data-state={state}
//         data-step={step}
//         className={cn("relative flex w-full items-start gap-6", className)}
//         {...props}
//       >
//         {typeof children === "function"
//           ? children({ state, step, isClickable })
//           : children}
//       </div>
//     </StepperItemContext.Provider>
//   );
// }

// type StepperSeparatorProps = React.HTMLAttributes<HTMLDivElement> & {
//   // 选填：在调用端传 orientation（若你要显式覆盖）
//   orientation?: "vertical" | "horizontal";
//   dataState?: string;
// };

// export function StepperSeparator({
//   className,
//   orientation: _orientation,
//   ...props
// }: StepperSeparatorProps) {
//   // 尝试从上下文获取 orientation，若没有则用传入的值
//   // 你的实现里如果没有像 useStepper 那样暴露 orientation，就简单用 data 属性判断
//   const orientation = _orientation ?? "vertical";

//   // 横向布局时，线条需要横向，竖向时需要纵向
//   const base = "block shrink-0 rounded-full"; // 可见的线条基础

//   const cls =
//     orientation === "horizontal"
//       ? [
//           base,
//           "h-0.5 w-[110%]", // 水平线，长度充满父容器
//           "bg-muted",
//           "data-[state=completed]:bg-primary",
//           // 如果你有状态样式，可以在这里继续扩展
//           "",
//         ].join(" ")
//       : [
//           base,
//           "w-0.5 h-[110%]", // 垂直线，高度占满
//           "bg-muted",
//           "data-[state=completed]:bg-primary",
//         ].join(" ");

//   return <div aria-hidden="true" className={cn(cls, className)} {...props} />;
// }

// type StepperTriggerProps = React.HTMLAttributes<HTMLDivElement> & {
//   step: number;
//   asChild?: boolean;
//   children: React.ReactNode;
// };

// export function StepperTrigger({
//   step,
//   asChild,
//   // children,
//   className,
//   ...props
// }: StepperTriggerProps) {
//   const { goToStep, canGoToStep, isTransitioning } = useStepper();
//   const { state } = useStepItem();
//   const clickable = canGoToStep(step);

//   if (asChild) {
//     const child = React.Children.only(children) as React.ReactElement;
//     return React.cloneElement(child, {
//       onClick: () => goToStep(step),
//       disabled: !clickable || isTransitioning,
//       "aria-current": state === "active" ? "step" : undefined,
//       className: cn("z-10 shrink-0", className, child.props.className),
//     });
//   }

//   return (
//     <button
//       type="button"
//       aria-current={state === "active" ? "step" : undefined}
//       data-state={state}
//       disabled={isTransitioning || !clickable}
//       onClick={() => goToStep(step)}
//       className={cn("z-10 shrink-0", className)}
//       {...props}
//     >
//       {children}
//     </button>
//   );
// }

// export function StepperTitle({
//   className,
//   ...props
// }: React.HTMLAttributes<HTMLDivElement>) {
//   const { state } = useStepItem();
//   return (
//     <div
//       data-state={state}
//       className={cn(
//         "text-sm font-semibold transition lg:text-base",
//         state === "active" && "text-primary",
//         className,
//       )}
//       {...props}
//     />
//   );
// }

// export function StepperDescription({
//   className,
//   ...props
// }: React.HTMLAttributes<HTMLDivElement>) {
//   const { state } = useStepItem();
//   return (
//     <div
//       data-state={state}
//       className={cn(
//         "text-xs text-muted-foreground transition lg:text-sm",
//         state === "active" && "text-primary",
//         className,
//       )}
//       {...props}
//     />
//   );
// }
