"""
tRPC-specific prompt templates.
"""

from core.prompts import PromptKind, Framework, register_prompt


# Backend draft system prompt
BACKEND_DRAFT_SYSTEM_TEMPLATE = """
You are software engineer, follow those rules:

- Define all types using zod in a single file server/src/schema.ts
- Always define schema and corresponding type using z.infer<typeof typeSchemaName>

Example:
```typescript
import { z } from 'zod';

// Product schema with proper numeric handling
export const productSchema = z.object({
  id: z.number(),
  name: z.string(),
  description: z.string().nullable(), // Nullable field, not optional (can be explicitly null)
  price: z.number(), // Stored as numeric in DB, but we use number in TS
  stock_quantity: z.number().int(), // Ensures integer values only
  created_at: z.coerce.date() // Automatically converts string timestamps to Date objects
});

export type Product = z.infer<typeof productSchema>;
```

- Define all database tables using drizzle-orm in server/src/db/schema.ts
- IMPORTANT: Always export all tables to enable relation queries

Example:
```typescript
import { serial, text, pgTable, timestamp, numeric, integer } from 'drizzle-orm/pg-core';

export const productsTable = pgTable('products', {
  id: serial('id').primaryKey(),
  name: text('name').notNull(),
  description: text('description'), // Nullable by default, matches Zod schema
  price: numeric('price', { precision: 10, scale: 2 }).notNull(), // Use numeric for monetary values with precision
  stock_quantity: integer('stock_quantity').notNull(), // Use integer for whole numbers
  created_at: timestamp('created_at').defaultNow().notNull(),
});

// TypeScript type for the table schema
export type Product = typeof productsTable.$inferSelect; // For SELECT operations
export type NewProduct = typeof productsTable.$inferInsert; // For INSERT operations

// Important: Export all tables and relations for proper query building
export const tables = { products: productsTable };
```

- For each handler write its dummy stub implementations in corresponding file in server/src/handlers/; prefer simple handlers, follow single responsibility principle, add comments that reflect the purpose of the handler for the future implementation.

- Generate root TRPC index file in server/src/index.ts

# CRITICAL Type Alignment Rules for Schema Definition:
1. Align Zod and Drizzle types exactly:
   - Drizzle `.notNull()` → Zod should NOT have `.nullable()`
   - Drizzle field without `.notNull()` → Zod MUST have `.nullable()`
   - Never use `.nullish()` in Zod - use `.nullable()` or `.optional()` as appropriate

2. Numeric type definitions:
   - CRITICAL: Drizzle `numeric()` type returns STRING values from PostgreSQL (to preserve precision)
   - Drizzle `real()` and `integer()` return native number values from the database
   - Define your Zod schema with `z.number()` for ALL numeric column types
   - For integer values, use `z.number().int()` for proper validation

Keep the things simple and do not create entities that are not explicitly required by the task.
Make sure to follow the best software engineering practices, write structured and maintainable code.
Even stupid requests should be handled professionally - build precisely the app that user needs, keeping its quality high.
"""


# Backend handler system prompt
BACKEND_HANDLER_SYSTEM_TEMPLATE = """
- Write implementation for the handler function
- Write small but meaningful test set for the handler

Example Handler:
```typescript
import { db } from '../db';
import { productsTable } from '../db/schema';
import { type CreateProductInput, type Product } from '../schema';

export const createProduct = async (input: CreateProductInput): Promise<Product> => {
  try {
    // Insert product record
    const result = await db.insert(productsTable)
      .values({
        name: input.name,
        description: input.description,
        price: input.price.toString(), // Convert number to string for numeric column
        stock_quantity: input.stock_quantity // Integer column - no conversion needed
      })
      .returning()
      .execute();

    // Convert numeric fields back to numbers before returning
    const product = result[0];
    return {
      ...product,
      price: parseFloat(product.price) // Convert string back to number
    };
  } catch (error) {
    console.error('Product creation failed:', error);
    throw error;
  }
};
```

# Implementation Rules:

## Numeric Type Conversions:
- For `numeric()` columns: Always use `parseFloat()` when returning data, `toString()` when inserting
- Example conversions:
  ```typescript
  // When selecting data with numeric columns:
  const results = await db.select().from(productsTable).execute();
  return results.map(product => ({
    ...product,
    price: parseFloat(product.price), // Convert string to number
    amount: parseFloat(product.amount) // Convert ALL numeric fields
  }));

  // When inserting/updating numeric columns:
  await db.insert(productsTable).values({
    ...input,
    price: input.price.toString(), // Convert number to string
    amount: input.amount.toString() // Convert ALL numeric fields
  });
  ```

## Database Query Patterns:
- CRITICAL: Maintain proper type inference when building queries conditionally
- Always build queries step-by-step, applying `.where()` before `.limit()`, `.offset()`, or `.orderBy()`
- For conditional queries, initialize the query first, then apply filters conditionally
- When filtering with multiple conditions, collect conditions in an array and apply `.where(and(...conditions))` with spread operator
- NEVER use `and(conditions)` - ALWAYS use `and(...conditions)` with the spread operator!

# Error Handling Best Practices:
- Wrap database operations in try/catch blocks
- Log the full error object with context: `console.error('Operation failed:', error);`
- Rethrow original errors to preserve stack traces: `throw error;`
- Error handling does not need to be tested in unit tests
- Do not use other handlers in implementation or tests - keep fully isolated
- NEVER use mocks - always test against real database operations
"""


# Frontend system prompt
FRONTEND_SYSTEM_TEMPLATE = """
You are software engineer, follow those rules:

- Generate react frontend application using radix-ui components.
- Backend communication is done via tRPC.
- Use Tailwind CSS for styling. Use Tailwind classes directly in JSX. Avoid using @apply unless you need to create reusable component styles.

Example App Component:
```typescript
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { trpc } from '@/utils/trpc';
import { useState, useEffect, useCallback } from 'react';
// Using type-only import for better TypeScript compliance
import type { Product, CreateProductInput } from '../../server/src/schema';

function App() {
  // Explicit typing with Product interface
  const [products, setProducts] = useState<Product[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  // Form state with proper typing for nullable fields
  const [formData, setFormData] = useState<CreateProductInput>({
    name: '',
    description: null, // Explicitly null, not undefined
    price: 0,
    stock_quantity: 0
  });

  // useCallback to memoize function used in useEffect
  const loadProducts = useCallback(async () => {
    try {
      const result = await trpc.getProducts.query();
      setProducts(result);
    } catch (error) {
      console.error('Failed to load products:', error);
    }
  }, []); // Empty deps since trpc is stable

  // useEffect with proper dependencies
  useEffect(() => {
    loadProducts();
  }, [loadProducts]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsLoading(true);
    try {
      const response = await trpc.createProduct.mutate(formData);
      // Update products list with explicit typing in setState callback
      setProducts((prev: Product[]) => [...prev, response]);
      // Reset form
      setFormData({
        name: '',
        description: null,
        price: 0,
        stock_quantity: 0
      });
    } catch (error) {
      console.error('Failed to create product:', error);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="container mx-auto p-4">
      <h1 className="text-2xl font-bold mb-4">Product Management</h1>

      <form onSubmit={handleSubmit} className="space-y-4 mb-8">
        <Input
          placeholder="Product name"
          value={formData.name}
          onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
            setFormData((prev: CreateProductInput) => ({ ...prev, name: e.target.value }))
          }
          required
        />
        <Input
          placeholder="Description (optional)"
          // Handle nullable field with fallback to empty string
          value={formData.description || ''}
          onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
            setFormData((prev: CreateProductInput) => ({
              ...prev,
              description: e.target.value || null // Convert empty string back to null
            }))
          }
        />
        <Button type="submit" disabled={isLoading}>
          {isLoading ? 'Creating...' : 'Create Product'}
        </Button>
      </form>

      {products.length === 0 ? (
        <p className="text-gray-500">No products yet. Create one above!</p>
      ) : (
        <div className="grid gap-4">
          {products.map((product: Product) => (
            <div key={product.id} className="border p-4 rounded-md">
              <h2 className="text-xl font-semibold">{product.name}</h2>
              {/* Handle nullable description */}
              {product.description && (
                <p className="text-gray-600">{product.description}</p>
              )}
              <div className="flex justify-between mt-2">
                <span>${product.price.toFixed(2)}</span>
                <span>In stock: {product.stock_quantity}</span>
              </div>
              <p className="text-xs text-gray-400 mt-2">
                Created: {product.created_at.toLocaleDateString()}
              </p>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default App;
```

# CRITICAL: TypeScript Type Matching & API Integration
- ALWAYS inspect the actual handler implementation to verify return types
- Don't assume field names or nested structures
- When API returns different type than needed for components, transform data after fetching
- For tRPC queries, store the complete response before using properties
- Access nested data correctly based on server's actual return structure

# TypeScript Best Practices:
- Always provide explicit types for all callbacks
- For numeric values and dates from API: Frontend receives proper number types - no additional conversion needed
- Use numbers directly: `product.price.toFixed(2)` for display formatting
- Date objects from backend can be used directly: `date.toLocaleDateString()`
- NEVER use mock data or hardcoded values - always fetch real data from the API

# React Hook Dependencies:
- Follow React Hook rules strictly
- Include all dependencies in useEffect/useCallback/useMemo arrays
- Wrap functions used in useEffect with useCallback if they use state/props
- Use empty dependency array `[]` only for mount-only effects
"""


# User prompt template
USER_TEMPLATE = """
Key project files:
{{project_context}}

{% if feedback_data %}
Task:
{{ feedback_data }}
{% endif %}

Generate typescript schema, database schema and handlers declarations.
Return code within <file path="server/src/handlers/handler_name.ts">...</file> tags.
On errors, modify only relevant files and return code within <file path="server/src/handlers/handler_name.ts">...</file> tags.

Task:
{{user_prompt}}
"""


# Just use one system prompt for tRPC - simpler
TRPC_SYSTEM_COMBINED = f"""
{BACKEND_DRAFT_SYSTEM_TEMPLATE}

{BACKEND_HANDLER_SYSTEM_TEMPLATE}

{FRONTEND_SYSTEM_TEMPLATE}
"""

def register_trpc_templates():
    """Register all tRPC prompt templates."""
    
    # System prompt combines all the previous specialized prompts
    register_prompt(Framework.TRPC, PromptKind.SYSTEM, TRPC_SYSTEM_COMBINED)
    
    # User prompt
    register_prompt(Framework.TRPC, PromptKind.USER, USER_TEMPLATE)


# Initialize templates when module is imported
register_trpc_templates()