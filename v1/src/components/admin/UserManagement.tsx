// src/components/admin/UserManagement.tsx
//
// 用户 / Legal Entity 管理（admin.db）。只有 leader 能看到；真正的权限校验在后端
// 每个 admin_* 命令里（用 Windows 登录名查共享盘上的 admin.db），这里的显示判断只是体验。

import { useForm } from "@tanstack/react-form";
import { useCallback, useEffect, useState } from "react";
import {
  type AdminOverview,
  type AppUserRow,
  commands,
  type LegalEntityRow,
} from "@/bindings";
import { TextField } from "@/components/query/fields";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { toast } from "@/components/ui/toast";

/** Select 的值不能是 null，用它代表「全 LE 汇总」（default_le_code 为 NULL） */
const ALL = "__all";

type Result =
  | { status: "ok"; data: AdminOverview }
  | { status: "error"; error: string };

function LeSelect({
  value,
  les,
  onChange,
  className,
}: {
  value: string;
  les: LegalEntityRow[];
  onChange: (v: string) => void;
  className?: string;
}) {
  const items = [
    { value: ALL, label: "全部 LE（汇总）" },
    ...les.map((l) => ({
      value: l.leCode,
      label: l.isActive ? l.leCode : `${l.leCode}（已停用）`,
    })),
  ];
  return (
    <Select items={items} value={value} onValueChange={(v) => v && onChange(v)}>
      <SelectTrigger className={className}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {items.map((i) => (
          <SelectItem key={i.value} value={i.value}>
            {i.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

function UserRow({
  user,
  les,
  save,
}: {
  user: AppUserRow;
  les: LegalEntityRow[];
  save: (patch: Partial<AppUserRow>) => void;
}) {
  const [name, setName] = useState(user.fullName);
  useEffect(() => setName(user.fullName), [user.fullName]);

  return (
    <TableRow className={user.isActive ? undefined : "text-muted-foreground"}>
      <TableCell className="font-mono">{user.username}</TableCell>
      <TableCell>
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={() =>
            name.trim() && name !== user.fullName && save({ fullName: name })
          }
          className="w-44"
        />
      </TableCell>
      <TableCell>
        <LeSelect
          value={user.defaultLeCode ?? ALL}
          les={les}
          onChange={(v) => save({ defaultLeCode: v === ALL ? null : v })}
          className="w-44"
        />
      </TableCell>
      <TableCell>
        <Checkbox
          checked={user.isActive}
          onCheckedChange={(c) => save({ isActive: c === true })}
        />
      </TableCell>
      <TableCell>
        <Checkbox
          checked={user.isLeader}
          onCheckedChange={(c) => save({ isLeader: c === true })}
        />
      </TableCell>
    </TableRow>
  );
}

function LeRow({
  le,
  setActive,
  rename,
}: {
  le: LegalEntityRow;
  setActive: (active: boolean) => void;
  rename: (newCode: string) => void;
}) {
  const [editing, setEditing] = useState<string | null>(null);
  return (
    <TableRow className={le.isActive ? undefined : "text-muted-foreground"}>
      <TableCell className="font-mono">
        {editing === null ? (
          le.leCode
        ) : (
          <Input
            autoFocus
            value={editing}
            onChange={(e) => setEditing(e.target.value)}
            className="w-28"
          />
        )}
      </TableCell>
      <TableCell>
        <Checkbox
          checked={le.isActive}
          onCheckedChange={(c) => setActive(c === true)}
        />
      </TableCell>
      <TableCell className="flex gap-1">
        {editing === null ? (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => setEditing(le.leCode)}
          >
            改名
          </Button>
        ) : (
          <>
            <Button
              type="button"
              size="sm"
              onClick={() => {
                rename(editing.trim());
                setEditing(null);
              }}
            >
              保存
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => setEditing(null)}
            >
              取消
            </Button>
          </>
        )}
      </TableCell>
    </TableRow>
  );
}

export default function UserManagement() {
  const [isLeader, setIsLeader] = useState(false);
  const [data, setData] = useState<AdminOverview | null>(null);

  const apply = useCallback(async (p: Promise<Result>, okMsg?: string) => {
    const r = await p;
    if (r.status === "ok") {
      setData(r.data);
      if (okMsg) toast.add({ type: "success", title: okMsg });
    } else {
      toast.add({ type: "error", title: r.error });
    }
    return r.status === "ok";
  }, []);

  useEffect(() => {
    void commands.currentUserInfo().then((r) => {
      if (r.status === "ok" && r.data.isLeader) {
        setIsLeader(true);
        void apply(commands.adminOverview());
      }
    });
  }, [apply]);

  const userForm = useForm({
    defaultValues: {
      username: "",
      fullName: "",
      defaultLe: ALL,
      isLeader: false,
    },
    onSubmit: async ({ value, formApi }) => {
      const ok = await apply(
        commands.adminAddUser({
          username: value.username,
          fullName: value.fullName,
          defaultLeCode: value.defaultLe === ALL ? null : value.defaultLe,
          isLeader: value.isLeader,
        }),
        "已添加用户"
      );
      if (ok) formApi.reset();
    },
  });

  const leForm = useForm({
    defaultValues: { leCode: "" },
    onSubmit: async ({ value, formApi }) => {
      if (await apply(commands.adminAddLe(value.leCode), "已添加 LE"))
        formApi.reset();
    },
  });

  if (!isLeader || !data) return null;

  const saveUser = (u: AppUserRow, patch: Partial<AppUserRow>) => {
    const n = { ...u, ...patch };
    void apply(
      commands.adminUpdateUser({
        id: n.id,
        fullName: n.fullName,
        defaultLeCode: n.defaultLeCode,
        isActive: n.isActive,
        isLeader: n.isLeader,
      })
    );
  };

  return (
    <div className="mx-auto flex max-w-[1000px] flex-col gap-4 p-6">
      <Card>
        <CardHeader>
          <CardTitle>用户管理</CardTitle>
          <CardDescription>
            用户名是 Windows 登录名（自动转大写）。修改立即写入共享盘上的
            admin.db，其他用户下次启动程序时生效。
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>用户名</TableHead>
                <TableHead>姓名</TableHead>
                <TableHead>默认 LE</TableHead>
                <TableHead>有效</TableHead>
                <TableHead>Leader</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.users.map((u) => (
                <UserRow
                  key={u.id}
                  user={u}
                  les={data.legalEntities}
                  save={(patch) => saveUser(u, patch)}
                />
              ))}
            </TableBody>
          </Table>

          <form
            className="flex flex-wrap items-end gap-3 border-t pt-3"
            onSubmit={(e) => {
              e.preventDefault();
              void userForm.handleSubmit();
            }}
          >
            <userForm.Field name="username">
              {(f) => (
                <TextField
                  field={f}
                  label="新用户名"
                  placeholder="如 M241452"
                />
              )}
            </userForm.Field>
            <userForm.Field name="fullName">
              {(f) => <TextField field={f} label="姓名" />}
            </userForm.Field>
            <userForm.Field name="defaultLe">
              {(f) => (
                <div className="flex flex-col gap-1">
                  <Label className="text-muted-foreground">默认 LE</Label>
                  <LeSelect
                    value={f.state.value}
                    les={data.legalEntities.filter((l) => l.isActive)}
                    onChange={f.handleChange}
                    className="w-44"
                  />
                </div>
              )}
            </userForm.Field>
            <userForm.Field name="isLeader">
              {(f) => (
                <Label className="gap-2 pb-2 text-muted-foreground">
                  <Checkbox
                    checked={f.state.value}
                    onCheckedChange={(c) => f.handleChange(c === true)}
                  />
                  Leader
                </Label>
              )}
            </userForm.Field>
            <Button type="submit">添加用户</Button>
          </form>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Legal Entity</CardTitle>
          <CardDescription>
            LE
            代码同时是数据库文件名和共享盘目录名，只能是字母数字；已有数据库文件的
            LE 不能改名。停用前需先改掉有效用户的默认 LE。
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>LE 代码</TableHead>
                <TableHead>有效</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.legalEntities.map((l) => (
                <LeRow
                  key={l.leCode}
                  le={l}
                  setActive={(a) =>
                    void apply(commands.adminSetLeActive(l.leCode, a))
                  }
                  rename={(n) =>
                    void apply(commands.adminRenameLe(l.leCode, n), "已改名")
                  }
                />
              ))}
            </TableBody>
          </Table>

          <form
            className="flex items-end gap-3 border-t pt-3"
            onSubmit={(e) => {
              e.preventDefault();
              void leForm.handleSubmit();
            }}
          >
            <leForm.Field name="leCode">
              {(f) => (
                <TextField field={f} label="新 LE 代码" placeholder="如 1784" />
              )}
            </leForm.Field>
            <Button type="submit">添加 LE</Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
