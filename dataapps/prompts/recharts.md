# Recharts Integration Instructions

When integrating Recharts into this React Admin template:

## Template Context
- Tech Stack: React Admin 5.10.0, React 19.0.0, TypeScript, Vite, Material-UI 7.0.1
- Project Structure: `frontend/reactadmin/src/` with App.tsx, Layout.tsx, components/
- Data Provider: Uses simple-rest data provider for API integration

## Recharts Integration Steps

1. **Install**: add `recharts` to `package.json` in frontend/reactadmin directory

2. **Component Structure**: Create reusable chart components in `src/components/charts/` directory

3. **Essential Imports**: Use ResponsiveContainer, LineChart, BarChart, PieChart, XAxis, YAxis, CartesianGrid, Tooltip, Legend from recharts

4. **Data Integration**: ALWAYS use React Admin native hooks (`useGetList`, `useGetOne`, `useGetMany`) instead of `useState` and `useEffect`

5. **Dashboard Integration**: Add charts to React Admin dashboard or create dedicated dashboard component

6. **Responsive Design**: Always wrap charts in ResponsiveContainer for responsive behavior

7. **Theme Integration**: Use Material-UI's `useTheme` hook to match chart colors with the app theme

8. **TypeScript**: Define proper interfaces for chart data and props

## Key Requirements
- **CRITICAL**: Use ResponsiveContainer for ALL charts - this is mandatory for responsive behavior
- **CRITICAL**: Prefer React Admin hooks (`useGetList`, `useGetOne`, `useGetMany`) over `useDataProvider` with `useEffect`
- Handle loading and error states properly using React Admin's `isPending` and `error` from hooks
- Integrate with existing Material-UI theme using `useTheme` hook
- Follow TypeScript patterns with proper interfaces
- Connect to React Admin data provider using native hooks
- Maintain responsive design principles
- Create reusable chart components
- Use proper error boundaries and loading states

## Available Chart Types
LineChart, BarChart, PieChart, AreaChart, ScatterChart, RadarChart, ComposedChart, FunnelChart, RadialBarChart, Sankey, SunburstChart, Treemap

## Data Fetching Best Practices

### ✅ PREFERRED: Use React Admin Native Hooks
```tsx
// Use useGetList for fetching lists
const { data, isPending, error } = useGetList('orders', {
  pagination: { page: 1, perPage: 50 },
  sort: { field: 'created_at', order: 'ASC' }
});

// Use useGetOne for single records
const { data: record, isPending, error } = useGetOne('users', { id: userId });

// Use useGetMany for multiple specific records
const { data, isPending, error } = useGetMany('categories', { ids: [1, 2, 3] });

// Use useGetManyReference for related records
const { data, isPending, error } = useGetManyReference('comments', {
  target: 'post_id',
  id: postId,
  pagination: { page: 1, perPage: 10 }
});
```

### ❌ AVOID: Manual useEffect with useDataProvider
```tsx
// Avoid this pattern - it's more verbose and error-prone
const [data, setData] = useState([]);
const [loading, setLoading] = useState(true);
const dataProvider = useDataProvider();

useEffect(() => {
  dataProvider.getList('orders', { ... })
    .then(({ data }) => setData(data))
    .catch(error => setError(error))
    .finally(() => setLoading(false));
}, []);
```

## React Admin Chart Examples

### ✅ BEST PRACTICE: Line Chart with useGetList Hook
```tsx
import { useGetList } from 'react-admin';
import { useTheme, Card, CardContent, Typography } from '@mui/material';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer } from 'recharts';

interface OrderData {
  id: string;
  created_at: string;
  total: number;
  status: string;
}

interface ChartData {
  date: string;
  amount: number;
  orders: number;
}

export const OrdersChart = () => {
  const theme = useTheme();
  const { data, isPending, error } = useGetList<OrderData>('orders', {
    pagination: { page: 1, perPage: 50 },
    sort: { field: 'created_at', order: 'ASC' }
  });

  if (isPending) return <Typography>Loading...</Typography>;
  if (error) return <Typography color="error">Error loading orders data</Typography>;

  const chartData: ChartData[] = data?.map(order => ({
    date: new Date(order.created_at).toLocaleDateString(),
    amount: order.total,
    orders: 1
  })) || [];

  return (
    <Card>
      <CardContent>
        <Typography variant="h6" gutterBottom>Orders Over Time</Typography>
        <ResponsiveContainer width="100%" height={400}>
          <LineChart data={chartData}>
            <CartesianGrid strokeDasharray="3 3" stroke={theme.palette.divider} />
            <XAxis 
              dataKey="date" 
              stroke={theme.palette.text.secondary}
            />
            <YAxis stroke={theme.palette.text.secondary} />
            <Tooltip 
              contentStyle={{
                backgroundColor: theme.palette.background.paper,
                border: `1px solid ${theme.palette.divider}`,
                borderRadius: theme.shape.borderRadius
              }}
            />
            <Legend />
            <Line 
              type="monotone" 
              dataKey="amount" 
              stroke={theme.palette.primary.main} 
              strokeWidth={2}
              name="Order Amount"
            />
          </LineChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
};
```

### ✅ BEST PRACTICE: Bar Chart with Data Aggregation
```tsx
import { useGetList } from 'react-admin';
import { Card, CardContent, CardHeader, Typography, useTheme } from '@mui/material';
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer } from 'recharts';

interface ProductData {
  id: string;
  category: string;
  price: number;
  stock: number;
  name: string;
}

interface CategorySalesData {
  category: string;
  sales: number;
  count: number;
}

export const CategorySalesChart = () => {
  const theme = useTheme();
  const { data, isPending, error } = useGetList<ProductData>('products', {
    pagination: { page: 1, perPage: 100 }
  });

  if (isPending) return <Typography>Loading...</Typography>;
  if (error) return <Typography color="error">Error loading products data</Typography>;

  // Aggregate data by category
  const chartData: CategorySalesData[] = data?.reduce((acc: CategorySalesData[], product) => {
    const existing = acc.find(item => item.category === product.category);
    const salesValue = product.price * product.stock;
    
    if (existing) {
      existing.sales += salesValue;
      existing.count += 1;
    } else {
      acc.push({ 
        category: product.category, 
        sales: salesValue,
        count: 1
      });
    }
    return acc;
  }, []) || [];

  return (
    <Card>
      <CardHeader title="Sales by Category" />
      <CardContent>
        <ResponsiveContainer width="100%" height={400}>
          <BarChart data={chartData}>
            <CartesianGrid 
              strokeDasharray="3 3" 
              stroke={theme.palette.divider} 
            />
            <XAxis 
              dataKey="category" 
              stroke={theme.palette.text.secondary}
            />
            <YAxis stroke={theme.palette.text.secondary} />
            <Tooltip 
              formatter={(value: number) => [`$${value.toLocaleString()}`, 'Sales']}
              contentStyle={{
                backgroundColor: theme.palette.background.paper,
                border: `1px solid ${theme.palette.divider}`,
                borderRadius: theme.shape.borderRadius
              }}
            />
            <Legend />
            <Bar 
              dataKey="sales" 
              fill={theme.palette.primary.main}
              name="Total Sales"
              radius={[4, 4, 0, 0]}
            />
          </BarChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
};
```

### ✅ BEST PRACTICE: Pie Chart with useGetList (Replacing useDataProvider + useEffect)
## For Pie Chart, need to set itemStyle to the theme.palette.text.primary
```tsx
import { useGetList } from 'react-admin';
import { Card, CardContent, Typography, useTheme } from '@mui/material';
import { PieChart, Pie, Cell, Tooltip, Legend, ResponsiveContainer } from 'recharts';

interface UserData {
  id: string;
  active: boolean;
  name: string;
  email: string;
}

interface StatusData {
  name: string;
  value: number;
}

export const UserStatusChart = () => {
  const theme = useTheme();
  const { data, isPending, error } = useGetList<UserData>('users', {
    pagination: { page: 1, perPage: 1000 },
    sort: { field: 'created_at', order: 'DESC' }
  });

  if (isPending) return <Typography>Loading...</Typography>;
  if (error) return <Typography color="error">Error loading user data</Typography>;

  // Aggregate data by status
  const statusData: StatusData[] = data?.reduce((acc: StatusData[], user) => {
    const status = user.active ? 'Active' : 'Inactive';
    const existing = acc.find(item => item.name === status);
    
    if (existing) {
      existing.value += 1;
    } else {
      acc.push({ name: status, value: 1 });
    }
    return acc;
  }, []) || [];

  const COLORS = [
    theme.palette.success.main, // Active - green
    theme.palette.error.main    // Inactive - red
  ];

  return (
    <Card>
      <CardContent>
        <Typography variant="h6" gutterBottom>User Status Distribution</Typography>
        <ResponsiveContainer width="100%" height={400}>
          <PieChart>
            <Pie
              data={statusData}
              dataKey="value"
              nameKey="name"
              cx="50%"
              cy="50%"
              outerRadius={80}
              label={({ name, percent }) => `${name}: ${(percent * 100).toFixed(0)}%`}
              labelLine={false}
            >
              {statusData.map((entry, index) => (
                <Cell 
                  key={`cell-${index}`} 
                  fill={COLORS[index % COLORS.length]} 
                />
              ))}
            </Pie>
            <Tooltip 
              formatter={(value: number, name: string) => [value, name]}
              contentStyle={{
                backgroundColor: theme.palette.background.paper,
                border: `1px solid ${theme.palette.divider}`,
                borderRadius: theme.shape.borderRadius
              }}
              itemStyle={{
                color: theme.palette.text.primary,
              }}
            />
            <Legend />
          </PieChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
};
```

### ✅ BEST PRACTICE: Area Chart with Stacked Data
```tsx
import { useGetList } from 'react-admin';
import { Card, CardContent, Typography, useTheme } from '@mui/material';
import { AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer } from 'recharts';

interface OrderData {
  id: string;
  created_at: string;
  total: number;
  status: 'completed' | 'pending' | 'cancelled';
}

interface RevenueData {
  date: string;
  revenue: number;
  profit: number;
}

export const RevenueChart = () => {
  const theme = useTheme();
  const { data, isPending, error } = useGetList<OrderData>('orders', {
    pagination: { page: 1, perPage: 100 },
    sort: { field: 'created_at', order: 'ASC' },
    filter: { status: 'completed' } // Only include completed orders
  });

  if (isPending) return <Typography>Loading...</Typography>;
  if (error) return <Typography color="error">Error loading revenue data</Typography>;

  const chartData: RevenueData[] = data?.map(order => ({
    date: new Date(order.created_at).toLocaleDateString(),
    revenue: order.total,
    profit: order.total * 0.2 // Example: 20% profit margin
  })) || [];

  return (
    <Card>
      <CardContent>
        <Typography variant="h6" gutterBottom>Revenue & Profit Trends</Typography>
        <ResponsiveContainer width="100%" height={400}>
          <AreaChart data={chartData} margin={{ top: 20, right: 30, left: 0, bottom: 0 }}>
            <CartesianGrid 
              strokeDasharray="3 3" 
              stroke={theme.palette.divider} 
            />
            <XAxis 
              dataKey="date" 
              stroke={theme.palette.text.secondary}
            />
            <YAxis 
              stroke={theme.palette.text.secondary}
              tickFormatter={(value) => `$${value.toLocaleString()}`}
            />
            <Tooltip 
              formatter={(value: number, name: string) => [
                `$${value.toLocaleString()}`, 
                name
              ]}
              contentStyle={{
                backgroundColor: theme.palette.background.paper,
                border: `1px solid ${theme.palette.divider}`,
                borderRadius: theme.shape.borderRadius
              }}
            />
            <Legend />
            <Area 
              type="monotone" 
              dataKey="profit" 
              stackId="1" 
              stroke={theme.palette.secondary.main} 
              fill={theme.palette.secondary.main}
              fillOpacity={0.6}
              name="Profit"
            />
            <Area 
              type="monotone" 
              dataKey="revenue" 
              stackId="1" 
              stroke={theme.palette.primary.main} 
              fill={theme.palette.primary.main}
              fillOpacity={0.8}
              name="Revenue"
            />
          </AreaChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
};
```

### ✅ ADVANCED: ComposedChart with Multiple Data Sources
```tsx
import { useGetList, useGetManyReference } from 'react-admin';
import { Card, CardContent, Typography, useTheme } from '@mui/material';
import { ComposedChart, Line, Bar, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer } from 'recharts';

interface OrderData {
  id: string;
  created_at: string;
  total: number;
  customer_id: string;
}

export const SalesAnalyticsChart = () => {
  const theme = useTheme();
  const { data: orders, isPending: ordersLoading, error: ordersError } = useGetList<OrderData>('orders', {
    pagination: { page: 1, perPage: 30 },
    sort: { field: 'created_at', order: 'ASC' }
  });

  if (ordersLoading) return <Typography>Loading...</Typography>;
  if (ordersError) return <Typography color="error">Error loading analytics data</Typography>;

  // Group orders by date and calculate metrics
  const chartData = orders?.reduce((acc: any[], order) => {
    const date = new Date(order.created_at).toLocaleDateString();
    const existing = acc.find(item => item.date === date);
    
    if (existing) {
      existing.orders += 1;
      existing.revenue += order.total;
      existing.avgOrderValue = existing.revenue / existing.orders;
    } else {
      acc.push({
        date,
        orders: 1,
        revenue: order.total,
        avgOrderValue: order.total
      });
    }
    return acc;
  }, []) || [];

  return (
    <Card>
      <CardContent>
        <Typography variant="h6" gutterBottom>Sales Analytics</Typography>
        <ResponsiveContainer width="100%" height={400}>
          <ComposedChart data={chartData} margin={{ top: 20, right: 30, bottom: 20, left: 20 }}>
            <CartesianGrid strokeDasharray="3 3" stroke={theme.palette.divider} />
            <XAxis 
              dataKey="date" 
              stroke={theme.palette.text.secondary}
            />
            <YAxis 
              yAxisId="left"
              stroke={theme.palette.text.secondary}
            />
            <YAxis 
              yAxisId="right" 
              orientation="right"
              stroke={theme.palette.text.secondary}
            />
            <Tooltip 
              contentStyle={{
                backgroundColor: theme.palette.background.paper,
                border: `1px solid ${theme.palette.divider}`,
                borderRadius: theme.shape.borderRadius
              }}
            />
            <Legend />
            <Bar 
              yAxisId="left"
              dataKey="orders" 
              fill={theme.palette.primary.main}
              name="Number of Orders"
            />
            <Line 
              yAxisId="right"
              type="monotone" 
              dataKey="avgOrderValue" 
              stroke={theme.palette.secondary.main}
              strokeWidth={3}
              name="Avg Order Value ($)"
            />
          </ComposedChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
};
```

### ✅ PERFORMANCE TIP: Using useGetManyReference for Related Data
```tsx
import { useGetManyReference, useRecordContext } from 'react-admin';
import { Card, CardContent, Typography, useTheme } from '@mui/material';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';

interface CommentData {
  id: string;
  created_at: string;
  post_id: string;
  content: string;
}

export const PostCommentsChart = () => {
  const record = useRecordContext(); // Gets the current post record
  const theme = useTheme();
  
  const { data: comments, isPending, error } = useGetManyReference<CommentData>('comments', {
    target: 'post_id',
    id: record?.id,
    pagination: { page: 1, perPage: 100 },
    sort: { field: 'created_at', order: 'ASC' }
  });

  if (isPending) return <Typography>Loading...</Typography>;
  if (error) return <Typography color="error">Error loading comments</Typography>;

  // Group comments by date
  const chartData = comments?.reduce((acc: any[], comment) => {
    const date = new Date(comment.created_at).toLocaleDateString();
    const existing = acc.find(item => item.date === date);
    
    if (existing) {
      existing.comments += 1;
    } else {
      acc.push({ date, comments: 1 });
    }
    return acc;
  }, []) || [];

  return (
    <Card>
      <CardContent>
        <Typography variant="h6" gutterBottom>Comments Over Time</Typography>
        <ResponsiveContainer width="100%" height={250}>
          <LineChart data={chartData}>
            <CartesianGrid strokeDasharray="3 3" stroke={theme.palette.divider} />
            <XAxis dataKey="date" stroke={theme.palette.text.secondary} />
            <YAxis stroke={theme.palette.text.secondary} />
            <Tooltip 
              contentStyle={{
                backgroundColor: theme.palette.background.paper,
                border: `1px solid ${theme.palette.divider}`,
                borderRadius: theme.shape.borderRadius
              }}
            />
            <Line 
              type="monotone" 
              dataKey="comments" 
              stroke={theme.palette.primary.main} 
              strokeWidth={2}
            />
          </LineChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
};
```

### ✅ DASHBOARD INTEGRATION: Adding Charts to Dashboard
```tsx
// src/App.tsx
import { Admin, Resource } from 'react-admin';
import { Dashboard } from './Dashboard';
import { dataProvider } from './dataProvider';

export const App = () => (
  <Admin dataProvider={dataProvider} dashboard={Dashboard}>
    <Resource name="orders" />
    <Resource name="products" />
    <Resource name="users" />
  </Admin>
);
```

```tsx
// src/Dashboard.tsx  
import { Grid, Typography, Box } from '@mui/material';
import { OrdersChart } from './components/charts/OrdersChart';
import { CategorySalesChart } from './components/charts/CategorySalesChart';
import { UserStatusChart } from './components/charts/UserStatusChart';
import { RevenueChart } from './components/charts/RevenueChart';

export const Dashboard = () => (
  <Box sx={{ p: 2 }}>
    <Typography variant="h4" gutterBottom>
      Analytics Dashboard
    </Typography>
    <Grid container spacing={3}>
      <Grid size={{ xs: 12, md: 8 }}>
        <RevenueChart />
      </Grid>
      <Grid size={{ xs: 12, md: 4 }}>
        <UserStatusChart />
      </Grid>
      <Grid size={{ xs: 12, md: 6 }}>
        <OrdersChart />
      </Grid>
      <Grid size={{ xs: 12, md: 6 }}>
        <CategorySalesChart />
      </Grid>
    </Grid>
  </Box>
);
```

## Error Handling & Performance Best Practices

### ✅ ERROR BOUNDARIES: Wrap Charts in Error Boundaries
```tsx
import { ErrorBoundary } from 'react-error-boundary';
import { Typography, Card, CardContent, Button } from '@mui/material';

const ChartErrorFallback = ({ error, resetErrorBoundary }: any) => (
  <Card>
    <CardContent>
      <Typography color="error" variant="h6">Chart Error</Typography>
      <Typography variant="body2" sx={{ my: 1 }}>
        {error.message}
      </Typography>
      <Button onClick={resetErrorBoundary} variant="outlined">
        Retry
      </Button>
    </CardContent>
  </Card>
);

const ChartWithErrorBoundary = ({ children }: { children: React.ReactNode }) => (
  <ErrorBoundary
    FallbackComponent={ChartErrorFallback}
    onReset={() => window.location.reload()}
  >
    {children}
  </ErrorBoundary>
);

// Usage in Dashboard
<Grid size={{ xs: 12, md: 6 }}>
  <ChartWithErrorBoundary>
    <OrdersChart />
  </ChartWithErrorBoundary>
</Grid>
```

### ✅ ACCESSIBILITY: Enable Chart Accessibility
```tsx
// Add to all LineChart, BarChart, PieChart components
<LineChart data={chartData} accessibilityLayer>
  {/* Chart content */}
</LineChart>

// Or for more control:
<LineChart 
  data={chartData}
  title="Orders Over Time Chart"
  desc="Line chart showing order amounts over the past 50 days"
>
  {/* Chart content */}
</LineChart>
```

### ✅ PERFORMANCE: Conditional Rendering & Data Validation
```tsx
const OptimizedChart = () => {
  const { data, isPending, error } = useGetList('orders', {
    pagination: { page: 1, perPage: 50 },
    sort: { field: 'created_at', order: 'ASC' }
  });

  // Early returns for better performance
  if (isPending) return <ChartSkeleton />;
  if (error) return <ChartError error={error} />;
  if (!data || data.length === 0) return <EmptyChart message="No data available" />;

  // Validate data structure before rendering
  const validData = data.filter(item => 
    item.created_at && 
    typeof item.total === 'number' && 
    item.total >= 0
  );

  if (validData.length === 0) return <EmptyChart message="Invalid data format" />;

  // Rest of component...
};
```

## TypeScript Interfaces & Types

```tsx
// src/types/chart.types.ts
export interface BaseChartProps {
  height?: number;
  title?: string;
  showLegend?: boolean;
  className?: string;
}

export interface ChartData {
  date: string;
  value: number;
  category?: string;
}

export interface OrderData {
  id: string;
  created_at: string;
  total: number;
  status: 'pending' | 'completed' | 'cancelled';
  customer_id: string;
}

export interface ProductData {
  id: string;
  name: string;
  category: string;
  price: number;
  stock: number;
}

export interface UserData {
  id: string;
  name: string;
  email: string;
  active: boolean;
  created_at: string;
}

// Theme integration type
export interface ChartThemeProps {
  primaryColor: string;
  secondaryColor: string;
  backgroundColor: string;
  textColor: string;
  gridColor: string;
}
```

## Final Checklist

- ✅ Use `ResponsiveContainer` for ALL charts
- ✅ Prefer React Admin native hooks (`useGetList`, `useGetOne`, `useGetMany`) over `useDataProvider` + `useEffect`
- ✅ Handle `isPending`, `error`, and empty states properly
- ✅ Use TypeScript interfaces for all data structures
- ✅ Integrate with Material-UI theme using `useTheme` hook
- ✅ Add proper error boundaries around charts
- ✅ Enable accessibility with `accessibilityLayer` prop
- ✅ Format data properly before passing to charts
- ✅ Use consistent color schemes from theme
- ✅ Add meaningful legends and tooltips
- ✅ Export components for reusability
- ✅ Validate data before rendering **charts**