import {
  Create,
  SimpleForm,
  useResourceContext,
  useGetResourceLabel,
  useGetList,
  TextInput,
  NumberInput,
  DateInput,
  BooleanInput,
  required,
  email,
} from "react-admin";
import { useState, useEffect, cloneElement } from "react";

interface CreateGuesserProps {
  title?: string;
  resource?: string;
}

/**
 * Automatically generates input fields for creating new records by inferring from existing data.
 * Similar to EditGuesser and ShowGuesser but for create operations.
 */
export const CreateGuesser = (props: CreateGuesserProps) => {
  const resource = useResourceContext(props);
  const getResourceLabel = useGetResourceLabel();
  const resourceLabel = getResourceLabel(resource || "Resource", 1);

  // Fetch a sample record to infer the data structure
  const {
    data: records,
    isLoading,
    error,
  } = useGetList(resource || "", {
    pagination: { page: 1, perPage: 1 },
    sort: { field: "id", order: "ASC" },
    filter: {},
  });

  const [inferredInputs, setInferredInputs] = useState<JSX.Element[] | null>(
    null,
  );

  useEffect(() => {
    if (records && records.length > 0) {
      const sampleRecord = records[0];
      const inputs = getElementsFromRecords([sampleRecord]);
      setInferredInputs(inputs);

      // Log the inferred form for development purposes (like ShowGuesser does)
      console.group(
        `CreateGuesser: Guessed Create view for resource "${resource}"`,
      );
      console.log("Sample record used for inference:", sampleRecord);
      console.log("Inferred inputs:", inputs);
      console.groupEnd();
    }
  }, [records, resource]);

  // Show loading state
  if (isLoading) {
    return (
      <Create {...props} title={`Create ${resourceLabel}`}>
        <SimpleForm>
          <div>Loading...</div>
        </SimpleForm>
      </Create>
    );
  }

  // Show error state
  if (error) {
    return (
      <Create {...props} title={`Create ${resourceLabel}`}>
        <SimpleForm>
          <div>Error loading data to infer fields: {error.message}</div>
        </SimpleForm>
      </Create>
    );
  }

  // No data available
  if (!records || records.length === 0) {
    return (
      <Create {...props} title={`Create ${resourceLabel}`}>
        <SimpleForm>
          <div>
            No records available to infer form fields. Please create a record
            manually first.
          </div>
        </SimpleForm>
      </Create>
    );
  }

  return (
    <Create {...props} title={`Create ${resourceLabel}`}>
      <SimpleForm>
        {inferredInputs?.map((input, index) =>
          cloneElement(input, { key: input.props.source || index }),
        )}
      </SimpleForm>
    </Create>
  );
};

/**
 * Infers input components from records, similar to getElementsFromRecords
 * used by ShowGuesser and EditGuesser.
 */
const getElementsFromRecords = (
  records: Record<string, unknown>[],
): JSX.Element[] => {
  if (!records || records.length === 0) {
    return [];
  }

  const sampleRecord = records[0];
  const inputs: JSX.Element[] = [];

  Object.keys(sampleRecord).forEach((fieldName) => {
    const fieldValue = sampleRecord[fieldName];

    // Skip ID fields as they're typically auto-generated
    if (fieldName === "id") {
      return;
    }

    const input = getInputFromFieldValue(fieldName, fieldValue);
    if (input) {
      inputs.push(input);
    }
  });

  return inputs;
};

/**
 * Determines the appropriate input component based on field name and value.
 * Uses heuristics to map field types to React Admin input components.
 */
const getInputFromFieldValue = (
  fieldName: string,
  fieldValue: unknown,
): JSX.Element | null => {
  const validation = getValidationRules(fieldName, fieldValue);

  // Determine input type based on field value and name
  if (typeof fieldValue === "boolean") {
    return <BooleanInput source={fieldName} validate={validation} fullWidth />;
  }

  if (typeof fieldValue === "number") {
    return <NumberInput source={fieldName} validate={validation} fullWidth />;
  }

  if (fieldValue instanceof Date || isDateField(fieldName)) {
    return <DateInput source={fieldName} validate={validation} fullWidth />;
  }

  // For string fields, use heuristics to determine specific input types
  if (
    typeof fieldValue === "string" ||
    fieldValue === null ||
    fieldValue === undefined
  ) {
    if (isEmailField(fieldName)) {
      return (
        <TextInput
          source={fieldName}
          validate={validation}
          fullWidth
          type="email"
        />
      );
    }

    if (isUrlField(fieldName)) {
      return (
        <TextInput
          source={fieldName}
          validate={validation}
          fullWidth
          type="url"
        />
      );
    }

    if (isDateField(fieldName)) {
      return <DateInput source={fieldName} validate={validation} fullWidth />;
    }

    // Default to TextInput for string fields
    return <TextInput source={fieldName} validate={validation} fullWidth />;
  }

  // Fallback to TextInput for unknown types
  return <TextInput source={fieldName} validate={validation} fullWidth />;
};

/**
 * Determines validation rules based on field name and value.
 * Uses generic patterns rather than hardcoded field names.
 */
const getValidationRules = (name: string, value: unknown) => {
  const rules = [];

  // Required fields based on generic patterns
  if (isRequiredField(name, value)) {
    rules.push(required());
  }

  // Email validation
  if (isEmailField(name)) {
    rules.push(email());
  }

  return rules.length > 0 ? rules : undefined;
};

/**
 * Heuristics to determine if a field is required based on generic patterns.
 * Avoids hardcoded field names to make the component truly generic.
 */
const isRequiredField = (name: string, value: unknown): boolean => {
  // Skip ID fields as they're typically auto-generated
  if (name === "id") {
    return false;
  }

  // Fields that are typically required based on common patterns
  const requiredPatterns = [
    "name", // Generic name field
    "title", // Generic title field
    "email", // Email fields are usually required
    "id", // ID fields (but we skip them above)
  ];

  // Check if field name matches required patterns
  const matchesRequiredPattern = requiredPatterns.some((pattern) =>
    name.toLowerCase().includes(pattern.toLowerCase()),
  );

  // Also consider the value - if it's not null/undefined in the sample, it might be required
  const hasValue = value !== null && value !== undefined && value !== "";

  return matchesRequiredPattern || hasValue;
};

/**
 * Heuristics to determine if a field is an email field.
 */
const isEmailField = (name: string): boolean => {
  return name.toLowerCase().includes("email");
};

/**
 * Heuristics to determine if a field is a URL field.
 * Uses generic patterns to detect URL-related fields.
 */
const isUrlField = (name: string): boolean => {
  const urlPatterns = ["url", "link", "website", "homepage", "href"];
  return urlPatterns.some((pattern) =>
    name.toLowerCase().includes(pattern.toLowerCase()),
  );
};

/**
 * Heuristics to determine if a field is a date field.
 * Uses generic patterns to detect date-related fields.
 */
const isDateField = (name: string): boolean => {
  const datePatterns = [
    "date",
    "time",
    "created",
    "updated",
    "modified",
    "timestamp",
  ];
  return datePatterns.some((pattern) =>
    name.toLowerCase().includes(pattern.toLowerCase()),
  );
};
