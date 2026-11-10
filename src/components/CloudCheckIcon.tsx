import * as React from "react";

interface CloudCheckIconProps
  extends React.SVGProps<SVGSVGElement> {
  cloudColor?: string;
  checkColor?: string;
}

export default function CloudCheckIcon({
  cloudColor = "currentColor",
  checkColor = "#22c55e",
  ...props
}: CloudCheckIconProps) {
  return (
    <svg
      viewBox="0 0 256 256"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      {...props}
    >
      <path
        d="M80,128a80,80,0,1,1,80,80H72A56,56,0,1,1,85.92,97.74"
        stroke={cloudColor}
        strokeWidth="16"
        strokeLinecap="round"
        strokeLinejoin="round"
      />

      <polyline
        points="120 136 144 160 192 112"
        stroke={checkColor}
        strokeWidth="16"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}