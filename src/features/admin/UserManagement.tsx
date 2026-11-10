// src/components/admin/UserManagement.tsx
//
// 用户 / Legal Entity 管理（admin.db）。只有 leader 能看到；真正的权限校验在后端
// 每个 admin_* 命令里（用 Windows 登录名查共享盘上的 admin.db），这里的显示判断只是体验。

import { useForm } from "@tanstack/react-form";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  type AdminOverview,
  type AppUserRow,
  commands,
  type LegalEntityRow,
} from "@/bindings";
import { TextField } from "@/features/query/fields";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
import { Switch } from "@/components/ui/switch";
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
  disabled,
}: {
  value: string;
  les: LegalEntityRow[];
  onChange: (v: string) => void;
  className?: string;
  disabled?: boolean;
}) {
  const items = [
    { value: ALL, label: "全部 LE（汇总）" },
    ...les.map((l) => ({
      value: l.leCode,
      label: l.isActive ? l.leCode : `${l.leCode}（已停用）`,
    })),
  ];
  return (
    <Select items={items} value={value} disabled={disabled} onValueChange={(v) => v && onChange(v)}>
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
  busy,
}: {
  user: AppUserRow;
  les: LegalEntityRow[];
  save: (patch: Partial<AppUserRow>) => void;
  /** 该行有保存在途：先禁用开关，避免同一行并发保存时回滚互相覆盖 */
  busy: boolean;
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
          disabled={busy}
          className="w-44"
        />
      </TableCell>
      <TableCell>
        <Switch
          aria-label="有效"
          size="sm"
          checked={user.isActive}
          disabled={busy}
          onCheckedChange={(c) => save({ isActive: c })}
        />
      </TableCell>
      <TableCell>
        <Switch
          aria-label="Leader"
          size="sm"
          checked={user.isLeader}
          disabled={busy}
          onCheckedChange={(c) => save({ isLeader: c })}
        />
      </TableCell>
    </TableRow>
  );
}

function LeRow({
  le,
  setActive,
  rename,
  busy,
}: {
  le: LegalEntityRow;
  setActive: (active: boolean) => void;
  rename: (newCode: string) => void;
  busy: boolean;
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
        <Switch
          aria-label="有效"
          size="sm"
          checked={le.isActive}
          disabled={busy}
          onCheckedChange={setActive}
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
  const dataRef = useRef(data);
  dataRef.current = data;

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

  // 乐观更新：点击瞬间先改本地界面，后台再写共享盘上的 admin.db（网络盘，一次往返可能很慢）。
  // 成功用后端返回的整份数据对齐；失败回滚到点击前的快照。busy 记录在途的行，期间禁用该行控件，
  // 这样同一行不会出现"后一次点击拿着前一次的乐观状态当快照"而回滚错乱。
  const [busy, setBusy] = useState<ReadonlySet<string>>(new Set());
  const optimistic = useCallback(
    async (
      key: string,
      update: (d: AdminOverview) => AdminOverview,
      request: Promise<Result>,
    ) => {
      // 快照直接取闭包里的 data，不在 setState 回调里取：回调可能被 React 延后或重复调用
      const snapshot = dataRef.current;
      setData(snapshot ? update(snapshot) : snapshot);
      setBusy((b) => new Set(b).add(key));
      try {
        const r = await request;
        if (r.status === "ok") setData(r.data);
        else {
          setData(snapshot);
          toast.add({ type: "error", title: r.error });
        }
      } catch (e) {
        setData(snapshot);
        toast.add({ type: "error", title: String(e) });
      } finally {
        setBusy((b) => {
          const n = new Set(b);
          n.delete(key);
          return n;
        });
      }
    },
    [],
  );

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
    void optimistic(
      `u${u.id}`,
      (d) => ({ ...d, users: d.users.map((x) => (x.id === u.id ? n : x)) }),
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
                  busy={busy.has(`u${u.id}`)}
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
                <Label className="gap-2 pb-1.5 text-muted-foreground">
                  <Switch
                    size="sm"
                    checked={f.state.value}
                    onCheckedChange={f.handleChange}
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
                  busy={busy.has(`l${l.leCode}`)}
                  setActive={(a) =>
                    void optimistic(
                      `l${l.leCode}`,
                      (d) => ({
                        ...d,
                        legalEntities: d.legalEntities.map((x) =>
                          x.leCode === l.leCode ? { ...x, isActive: a } : x,
                        ),
                      }),
                      commands.adminSetLeActive(l.leCode, a),
                    )
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
